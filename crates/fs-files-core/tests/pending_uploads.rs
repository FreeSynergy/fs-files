//! The list of unfinished uploads.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::{env, reopen};
use fs_files_core::{Files, Transfer};

#[tokio::test]
async fn unfinished_uploads_are_listed_with_their_progress() {
    let e = env().await;
    let r = &e.repo;
    assert!(r.capabilities().is_supported("file.pendingUploads"));
    assert!(r.pending_uploads().await.unwrap().is_empty());
    let d = r.create_folder(None, "d").await.unwrap();
    let (first, id1) = r
        .start_upload(Some(d.id), "first.bin", "application/octet-stream")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    let (second, id2) = r
        .start_upload(None, "second.txt", "text/plain")
        .await
        .unwrap();
    first.write(0, b"12345").await.unwrap();

    let list = r.pending_uploads().await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        (list[0].id.as_str(), list[0].name.as_str()),
        (id1.as_str(), "first.bin")
    );
    assert_eq!((list[0].folder, list[0].received), (Some(d.id), 5));
    assert_eq!((list[1].id.as_str(), list[1].received), (id2.as_str(), 0));
    assert_eq!(list[1].media_type, "text/plain");
    assert!(list[0].started <= list[1].started);

    // finished and aborted uploads are gone
    first.finish().await.unwrap();
    assert_eq!(r.pending_uploads().await.unwrap().len(), 1);
    second.abort().await.unwrap();
    assert!(r.pending_uploads().await.unwrap().is_empty());
}

#[tokio::test]
async fn the_list_survives_a_restart_and_resuming_works_from_it() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let (r, _s) = reopen(dir.path(), false).await;
        let (up, id) = r.start_upload(None, "big.bin", "").await.unwrap();
        up.write(0, b"first half ").await.unwrap();
        id
    };
    let (r, _s) = reopen(dir.path(), false).await;
    let list = r.pending_uploads().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].id.as_str(), list[0].received), (id.as_str(), 11));
    let up = r.resume_upload(&list[0].id).await.unwrap();
    up.write(list[0].received, b"second half").await.unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(common::read_all(&r, f.id).await, b"first half second half");
    assert!(r.pending_uploads().await.unwrap().is_empty());
}
