//! Upload and download by path, unfinished uploads, and what a path may not be.
//!
//! Pattern: integration tests over the real core and real folders.

mod common;

use common::World;
use fs_components_widgets::transfer_list::{Action as TransferAction, Outcome as TransferOutcome};
use fs_components_workbenches::file_manager::{Ask, Confirm, FileProblem, FileSource, NameRule};
use fs_files_core::Transfer;

fn retry(key: &str) -> TransferOutcome {
    TransferOutcome::new(key, TransferAction::Retry)
}

fn dismiss(key: &str) -> TransferOutcome {
    TransferOutcome::new(key, TransferAction::Dismiss)
}

#[test]
fn an_upload_by_path_arrives_whole_under_its_name_and_is_not_marked() {
    let w = World::new();
    let path = w.outside().join("größe mit space.bin");
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    let mut s = w.state();
    s.begin_upload();
    s.set_input(path.to_str().unwrap());
    s.submit_input();
    assert_eq!(*s.ask(), Ask::Nothing, "{:?}", s.notice());
    assert_eq!(
        std::fs::read(w.root().join("größe mit space.bin")).unwrap(),
        data
    );
    assert!(
        std::fs::read(&path).unwrap() == data,
        "the source is untouched"
    );
    assert_eq!(s.transfers().len(), 1);
    assert!(s.rows().iter().any(|r| r.name == "größe mit space.bin"));
}

#[cfg(unix)]
#[test]
fn a_symlink_a_folder_or_nothing_cannot_be_uploaded_and_an_input_stays() {
    let w = World::new();
    let secret = w.outside().join("secret.txt");
    std::fs::write(&secret, b"secret").unwrap();
    let link = w.outside().join("link.txt");
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    for path in [
        link.to_str().unwrap(),
        w.outside().to_str().unwrap(),
        "",
        "   ",
        "/nothing/here/at/all.txt",
    ] {
        let said = w.adapter.upload(path, None, NameRule::Refuse);
        assert!(said.is_err(), "{path}");
        assert!(w.tree().is_empty(), "{path}");
    }
    let mut s = w.state();
    s.begin_upload();
    s.set_input(link.to_str().unwrap());
    s.submit_input();
    match s.ask() {
        Ask::Input(input) => assert!(input.error.as_deref().unwrap().contains("link")),
        other => panic!("{other:?}"),
    }
    assert!(w.tree().is_empty());
}

#[test]
fn a_download_never_replaces_and_a_folder_means_inside_it() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"bytes");
    let key = f.id.to_string();
    let out = w.outside();
    let taken = out.join("taken.txt");
    std::fs::write(&taken, b"keep me").unwrap();
    assert!(
        w.adapter
            .download(&key, taken.to_str().unwrap(), false)
            .is_err()
    );
    assert_eq!(std::fs::read(&taken).unwrap(), b"keep me");
    let place = w
        .adapter
        .download(&key, out.to_str().unwrap(), false)
        .unwrap();
    assert_eq!(std::fs::read(&place).unwrap(), b"bytes");
    assert!(place.ends_with("a.txt"));
    assert!(
        w.adapter
            .download(&key, out.to_str().unwrap(), false)
            .is_err(),
        "now taken"
    );
    let missing = out.join("no/such/folder/x.txt");
    assert!(
        w.adapter
            .download(&key, missing.to_str().unwrap(), false)
            .is_err()
    );
    #[cfg(unix)]
    {
        let aim = out.join("aim.txt");
        let link = out.join("dangling.txt");
        std::os::unix::fs::symlink(&aim, &link).unwrap();
        assert!(
            w.adapter
                .download(&key, link.to_str().unwrap(), false)
                .is_err()
        );
        assert!(!aim.exists(), "not written through the link");
    }
}

#[test]
fn an_unfinished_upload_shows_goes_on_from_where_it_was_and_arrives_whole() {
    let w = World::new();
    let data = vec![42u8; 500_000];
    let path = w.outside().join("big.bin");
    std::fs::write(&path, &data).unwrap();
    w.runtime.block_on(async {
        let (up, _) = w.repo.start_upload(None, "big.bin", "").await.unwrap();
        up.write(0, &data[..200_000]).await.unwrap();
    });
    let mut s = w.state();
    s.open_transfers();
    assert_eq!(s.transfers().len(), 1);
    assert_eq!(s.transfers()[0].shown.done, 200_000);
    let key = s.transfers()[0].shown.key.clone();
    s.transfer_outcome(&retry(&key));
    s.set_input(path.to_str().unwrap());
    s.submit_input();
    assert_eq!(*s.ask(), Ask::Nothing, "{:?}", s.notice());
    assert_eq!(std::fs::read(w.root().join("big.bin")).unwrap(), data);
    assert!(s.transfers().iter().all(|r| r.pending.is_none()));
    assert!(s.rows().iter().any(|r| r.name == "big.bin"));
}

#[test]
fn resume_refuses_another_name_or_a_smaller_file_and_what_arrived_stays() {
    let w = World::new();
    w.runtime.block_on(async {
        let (up, _) = w.repo.start_upload(None, "big.bin", "").await.unwrap();
        up.write(0, &[1u8; 1000]).await.unwrap();
    });
    let id = w.adapter.pending_uploads().unwrap()[0].id.clone();
    let other = w.outside().join("other.bin");
    std::fs::write(&other, vec![1u8; 2000]).unwrap();
    assert!(
        w.adapter
            .resume_upload(&id, other.to_str().unwrap())
            .is_err()
    );
    let small = w.outside().join("big.bin");
    std::fs::write(&small, vec![1u8; 10]).unwrap();
    assert!(
        w.adapter
            .resume_upload(&id, small.to_str().unwrap())
            .is_err()
    );
    assert_eq!(w.adapter.pending_uploads().unwrap()[0].received, 1000);
    assert!(w.tree().is_empty());
}

#[test]
fn dropping_an_unfinished_upload_is_two_acts() {
    let w = World::new();
    w.runtime.block_on(async {
        let (up, _) = w.repo.start_upload(None, "big.bin", "").await.unwrap();
        up.write(0, &[1u8; 1000]).await.unwrap();
    });
    let mut s = w.state();
    s.open_transfers();
    let key = s.transfers()[0].shown.key.clone();
    s.transfer_outcome(&dismiss(&key));
    assert!(matches!(s.ask(), Ask::Confirm(Confirm::DropUpload { .. })));
    assert_eq!(w.adapter.pending_uploads().unwrap().len(), 1);
    s.really();
    assert!(w.adapter.pending_uploads().unwrap().is_empty());
    assert!(s.transfers().is_empty());
}

#[test]
fn a_gone_file_is_not_found_and_a_key_the_core_never_gave_too() {
    let w = World::new();
    let said = w
        .adapter
        .download("../../etc/passwd", "x", false)
        .unwrap_err();
    assert_eq!(said.problem, FileProblem::NotFound);
    assert!(w.adapter.preview("not an id", 10).is_err());
}
