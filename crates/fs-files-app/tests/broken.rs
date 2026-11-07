//! An upload whose connection breaks in the middle, and the limit of a download that is written whole
//! from memory. Both over the real server; the break is made by a decorator that stops the server after
//! a number of chunks.

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use common::{Providers, Tripped, Tripwire, World};
use fs_components_workbenches::file_manager::{FileProblem, FileSource, NameRule};
use fs_files_app::FileAdapter;
use fs_files_core::{Entry, Files, Transfer};

const MIB: usize = 1 << 20;

#[test]
fn a_connection_that_breaks_mid_upload_fails_the_upload_and_keeps_what_arrived() {
    let after = 2;
    let w = World::behind(|repo, stop| {
        let mut p = Providers::of(repo);
        p.transfer = Arc::new(Tripped(Arc::new(Tripwire {
            repo: repo.clone(),
            stop: stop.clone(),
            after,
            written: Arc::new(AtomicUsize::new(0)),
        })));
        p
    });
    let data = vec![9u8; 5 * MIB + 3];
    let path = w.outside().join("big.bin");
    std::fs::write(&path, &data).unwrap();
    let err = w
        .adapter
        .upload(path.to_str().unwrap(), None, NameRule::Refuse)
        .unwrap_err();
    assert!(!err.to_string().is_empty());
    let page = w.runtime.block_on(w.repo.list(None, None)).unwrap();
    assert!(
        !page
            .items
            .iter()
            .any(|e| matches!(e, Entry::File(f) if f.name == "big.bin")),
        "no half file"
    );
    let pending = w.runtime.block_on(w.repo.pending_uploads()).unwrap();
    assert_eq!(
        pending.len(),
        1,
        "what arrived stays as an unfinished upload"
    );
    let got = pending[0].received;
    assert!(got > 0 && got < data.len() as u64, "{got}");
    // The upload goes on from there with a new connection to the same provider.
    let up = w
        .runtime
        .block_on(w.repo.resume_upload(&pending[0].id))
        .unwrap();
    let from = usize::try_from(got).unwrap();
    let file = w.runtime.block_on(async {
        up.write(got, &data[from..]).await.unwrap();
        up.finish().await.unwrap()
    });
    assert_eq!(file.size, data.len() as u64);
}

#[test]
fn a_file_above_the_limit_is_refused_before_anything_is_written() {
    let w = World::new();
    let made = w.file(None, "five.bin", b"12345");
    let adapter = FileAdapter::new(
        Arc::new(fs_files_client::RpcFiles::connect(&w.socket).unwrap()),
        Arc::new(fs_files_client::RpcFiles::connect(&w.socket).unwrap()),
        w.runtime.handle().clone(),
    )
    .with_most_in_memory(4);
    let out = w.outside();
    let err = adapter
        .download(&made.id.to_string(), out.to_str().unwrap(), false)
        .unwrap_err();
    assert_eq!(err.problem, FileProblem::Failed);
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    assert_eq!(
        FileAdapter::MOST_IN_MEMORY,
        2 * 1024 * 1024 * 1024,
        "the real limit is 2 GiB"
    );
}
