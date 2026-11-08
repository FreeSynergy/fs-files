//! `FileFeed` (`file.subscribe`): every change of a file or folder is told, nothing else is.
#![allow(clippy::assert_is_empty, clippy::items_after_statements)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons; tests: a local `use` next to its use reads clearer

mod common;

use std::sync::{Arc, Mutex};

use common::{env, upload};
use fs_files_core::{FileChange, FileFeed, FileListener, Files, Transfer};

#[derive(Default)]
struct Seen(Mutex<Vec<FileChange>>);

impl FileListener for Seen {
    fn changed(&self, change: &FileChange) {
        self.0.lock().unwrap().push(*change);
    }
}

impl Seen {
    fn take(&self) -> Vec<FileChange> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[tokio::test]
async fn every_change_of_a_file_and_a_folder_is_told_in_order() {
    let e = env().await;
    let r = &e.repo;
    let seen = Arc::new(Seen::default());
    let _subscription = r.subscribe(seen.clone());

    let folder = r.create_folder(None, "Trips").await.unwrap();
    let file = upload(r, Some(folder.id), "a.txt", "text/plain", b"hello").await;
    r.rename_file(file.id, "b.txt").await.unwrap();
    r.trash(file.id).await.unwrap();
    r.purge(file.id).await.unwrap();
    r.delete_folder(folder.id).await.unwrap();

    use FileChange::{Created, Deleted, Updated};
    assert_eq!(
        seen.take(),
        [
            Created(folder.id),
            Created(file.id),
            Updated(file.id),
            Updated(file.id),
            Deleted(file.id),
            Deleted(folder.id),
        ]
    );
}

#[tokio::test]
async fn an_unfinished_upload_is_not_a_change_and_a_refused_write_is_not_told() {
    let e = env().await;
    let r = &e.repo;
    let seen = Arc::new(Seen::default());
    let _subscription = r.subscribe(seen.clone());

    let (up, _) = r.start_upload(None, "big.bin", "").await.unwrap();
    up.write(0, b"part").await.unwrap();
    up.abort().await.unwrap();
    assert!(r.create_folder(None, "").await.is_err());
    assert_eq!(seen.take(), []);
}

#[tokio::test]
async fn dropping_the_subscription_ends_it_and_each_listener_hears_alone() {
    let e = env().await;
    let r = &e.repo;
    let (first, second) = (Arc::new(Seen::default()), Arc::new(Seen::default()));
    let kept = r.subscribe(first.clone());
    let dropped = r.subscribe(second.clone());
    r.create_folder(None, "One").await.unwrap();
    drop(dropped);
    r.create_folder(None, "Two").await.unwrap();
    assert_eq!(first.take().len(), 2);
    assert_eq!(second.take().len(), 1);
    drop(kept);
    r.create_folder(None, "Three").await.unwrap();
    assert!(first.take().is_empty());
}

#[tokio::test]
async fn a_rescan_tells_the_files_it_takes_over() {
    let e = env().await;
    let r = &e.repo;
    let seen = Arc::new(Seen::default());
    let _subscription = r.subscribe(seen.clone());
    std::fs::write(e.dir.path().join("blobs").join("found.txt"), b"x").unwrap();
    let report = r.rescan(None).await.unwrap();
    assert_eq!(report.added.len(), 1);
    assert!(matches!(seen.take().as_slice(), [FileChange::Created(_)]));
}
