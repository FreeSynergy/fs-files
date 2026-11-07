//! What blocks the interface thread, and for how long. Not a pass or fail: run by hand with
//! `cargo test --release -p fs-files-app --test measure -- --ignored --nocapture`; the numbers are in
//! `DESIGN.md`.
//!
//! Pattern: measurement over the real core and a real folder, debug or release as asked.

mod common;

use std::time::Instant;

use common::World;
use fs_components_workbenches::file_manager::SortKey;

fn timed<T>(what: &str, work: impl FnOnce() -> T) -> T {
    let began = Instant::now();
    let answer = work();
    eprintln!("{what}: {:.2} s", began.elapsed().as_secs_f64());
    answer
}

#[test]
#[ignore = "measures, takes minutes"]
fn a_large_folder() {
    let count: usize = std::env::var("FS_FILES_COUNT").map_or(5000, |c| c.parse().unwrap());
    let w = World::new();
    for i in 0..count {
        std::fs::write(w.root().join(format!("file {i:06}.txt")), b"x").unwrap();
    }
    let mut s = w.state();
    timed(&format!("rescan of {count} new files"), || s.rescan());
    eprintln!("loaded {} of {count}", s.rows().len());
    timed("retry: list and tree read again, first page", || s.retry());
    timed("sort by size", || s.set_sort(SortKey::Size));
    timed("sort by name", || s.set_sort(SortKey::Name));
    timed("search one letter", || s.search("9"));
    timed("search on", || s.search("99"));
    timed("search off", || s.search(""));
    timed("select a file (reads the preview)", || {
        let key = s.rows()[0].key.clone();
        s.select(&key);
    });
    timed("one rename", || {
        s.begin_rename();
        s.set_input("renamed.txt");
        s.submit_input();
    });
    timed("Show more, once", || s.load_more());
    eprintln!("loaded {}", s.rows().len());
    timed("rescan with nothing new", || s.rescan());
}

#[test]
#[ignore = "measures"]
fn a_large_upload_and_download() {
    let w = World::new();
    let megabytes: usize = std::env::var("FS_FILES_MB").map_or(100, |c| c.parse().unwrap());
    let path = w.outside().join("big.bin");
    std::fs::write(&path, vec![7u8; megabytes * 1024 * 1024]).unwrap();
    let mut s = w.state();
    s.begin_upload();
    s.set_input(path.to_str().unwrap());
    timed(&format!("upload of {megabytes} MiB"), || s.submit_input());
    assert_eq!(s.rows().len(), 1);
    let key = s.rows()[0].key.clone();
    s.select(&key);
    let target = w.outside().join("back.bin");
    s.begin_download();
    s.set_input(target.to_str().unwrap());
    timed(&format!("download of {megabytes} MiB"), || s.submit_input());
    assert_eq!(
        std::fs::metadata(&target).unwrap().len(),
        (megabytes * 1024 * 1024) as u64
    );
}
