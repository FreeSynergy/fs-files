//! Data-loss cases found in review: an upload handle must never destroy a live file, and nobody
//! else's file may be moved, merged into or deleted because a record says so.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::disk::{on_disk, root};
use common::flaky::Flaky;
use common::{ME, env, names, upload};
use fs_files_core::{Error, FileRepo, Files, LocalFolder, Transfer};
use fs_store::{Store, TursoStore};

// ---- blocker 1: abort and stale sessions ----

#[cfg(feature = "s3")]
mod s3 {
    use super::*;
    use common::fake_s3::Fake;
    use common::read_all;
    use fs_files_core::FileFilter;
    use fs_files_core::{S3Backend, S3Options};

    async fn repo_on_s3() -> (FileRepo, Arc<Flaky>, Arc<Fake>) {
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        let flaky = Arc::new(Flaky {
            store,
            fail: Arc::default(),
            fail_delete: Arc::default(),
        });
        let bucket = Arc::new(Fake::default());
        let backend = Arc::new(S3Backend::new(bucket.clone(), &S3Options::default()).unwrap());
        let repo = FileRepo::with_backend(flaky.clone(), flaky.clone(), ME, "cloud", backend);
        (repo, flaky, bucket)
    }

    #[tokio::test]
    async fn abort_after_finish_keeps_the_file() {
        let (r, _f, bucket) = repo_on_s3().await;
        let (up, _) = r.start_upload(None, "a.txt", "text/plain").await.unwrap();
        up.write(0, b"precious").await.unwrap();
        let file = up.finish().await.unwrap();
        up.abort().await.unwrap(); // on the same handle: nothing to abort any more
        assert_eq!(read_all(&r, file.id).await, b"precious");
        assert_eq!(bucket.keys().len(), 1);
    }

    #[tokio::test]
    async fn a_stale_session_cannot_destroy_a_live_s3_file() {
        let (r, flaky, _bucket) = repo_on_s3().await;
        let (up, id) = r.start_upload(None, "a.txt", "text/plain").await.unwrap();
        up.write(0, b"precious").await.unwrap();
        flaky.fail_delete.store(true, Ordering::SeqCst);
        // the record is written, deleting the session fails
        let _ = up.finish().await;
        flaky.fail_delete.store(false, Ordering::SeqCst);
        let file = r.find(&FileFilter::default(), None).await.unwrap();
        assert_eq!(file.items.len(), 1);
        r.resume_upload(&id).await.unwrap().abort().await.unwrap();
        let listed = r.pending_uploads().await.unwrap();
        assert!(
            listed.is_empty(),
            "a finished upload is not pending: {listed:?}"
        );
        let id = match &file.items[0] {
            fs_files_core::Entry::File(f) => f.id,
            fs_files_core::Entry::Folder(_) => unreachable!(),
        };
        assert_eq!(read_all(&r, id).await, b"precious");
    }

    #[tokio::test]
    async fn finish_with_the_database_down_keeps_the_bytes() {
        let (r, flaky, _bucket) = repo_on_s3().await;
        let (up, _) = r.start_upload(None, "a.txt", "text/plain").await.unwrap();
        up.write(0, b"precious").await.unwrap();
        flaky.fail.store(true, Ordering::SeqCst);
        assert!(up.finish().await.is_err());
        flaky.fail.store(false, Ordering::SeqCst);
        let file = up
            .finish()
            .await
            .expect("the bytes are still there; finish again");
        assert_eq!(read_all(&r, file.id).await, b"precious");
    }
}

async fn local_flaky(dir: &std::path::Path) -> (FileRepo, Arc<Flaky>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let flaky = Arc::new(Flaky {
        store,
        fail: Arc::default(),
        fail_delete: Arc::default(),
    });
    let backend = Arc::new(LocalFolder::open(dir.join("blobs")).await.unwrap());
    (
        FileRepo::with_backend(flaky.clone(), flaky.clone(), ME, "main", backend),
        flaky,
    )
}

#[tokio::test]
async fn a_stale_session_cannot_destroy_a_live_local_file() {
    let dir = tempfile::tempdir().unwrap();
    let (r, flaky) = local_flaky(dir.path()).await;
    let (up, id) = r.start_upload(None, "a.txt", "text/plain").await.unwrap();
    up.write(0, b"precious").await.unwrap();
    flaky.fail_delete.store(true, Ordering::SeqCst);
    let _ = up.finish().await;
    flaky.fail_delete.store(false, Ordering::SeqCst);
    r.resume_upload(&id).await.unwrap().abort().await.unwrap();
    assert!(r.pending_uploads().await.unwrap().is_empty());
    assert_eq!(
        std::fs::read(dir.path().join("blobs/a.txt")).unwrap(),
        b"precious"
    );
    assert_eq!(names(&r, None).await, ["a.txt"]);
    // finishing the stale handle again hands out the file, once
    let again = up.finish().await;
    assert!(again.is_err() || again.unwrap().name == "a.txt");
}

// ---- blocker 2: only move, merge into or delete the recorded bytes ----

fn changed_outside<T: std::fmt::Debug>(r: fs_files_core::Result<T>) -> bool {
    r.as_ref()
        .err()
        .and_then(Error::problem)
        .is_some_and(|p| p.kind.ends_with("changed-outside"))
}

#[tokio::test]
async fn a_file_replaced_from_outside_is_not_trashed_or_purged() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "Foto.jpg", "image/jpeg", b"mine").await;
    std::fs::write(root(&e).join("Foto.jpg"), b"somebody else's, and larger").unwrap();
    assert!(changed_outside(r.trash(f.id).await));
    assert!(matches!(r.purge(f.id).await, Err(Error::Conflict(_))));
    assert_eq!(on_disk(&e, "Foto.jpg"), b"somebody else's, and larger");
    assert!(!r.get(f.id).await.unwrap().unwrap().trashed);
    // the same size, different bytes: the checksum decides
    let g = upload(r, None, "g.txt", "text/plain", b"12345").await;
    std::fs::write(root(&e).join("g.txt"), b"54321").unwrap();
    assert!(changed_outside(r.trash(g.id).await));
    assert!(changed_outside(r.rename_file(g.id, "h.txt").await));
    assert_eq!(on_disk(&e, "g.txt"), b"54321");
}

#[tokio::test]
async fn a_folder_rename_or_move_never_merges_into_an_existing_path() {
    let e = env().await;
    let r = &e.repo;
    let a = r.create_folder(None, "A").await.unwrap();
    let inside = upload(r, Some(a.id), "mine.txt", "text/plain", b"mine").await;
    // A vanished from the disk, and an unknown B with a file appeared
    std::fs::remove_dir_all(root(&e).join("A")).unwrap();
    std::fs::create_dir(root(&e).join("B")).unwrap();
    std::fs::write(root(&e).join("B/doc.txt"), b"theirs").unwrap();
    let err = r.rename_folder(a.id, "B").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err}");
    assert_eq!(r.get_folder(a.id).await.unwrap().unwrap().name, "A");
    assert_eq!(on_disk(&e, "B/doc.txt"), b"theirs");

    // move onto an unknown T/M while M is gone from the disk
    let t = r.create_folder(None, "T").await.unwrap();
    let m = r.create_folder(None, "M").await.unwrap();
    std::fs::remove_dir_all(root(&e).join("M")).unwrap();
    std::fs::create_dir(root(&e).join("T/M")).unwrap();
    std::fs::write(root(&e).join("T/M/x"), b"theirs").unwrap();
    let err = r.move_folder(m.id, Some(t.id)).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "{err}");
    assert_eq!(r.get_folder(m.id).await.unwrap().unwrap().parent, None);
    assert_eq!(on_disk(&e, "T/M/x"), b"theirs");
    let _ = inside;
}

#[tokio::test]
async fn restore_does_not_take_over_a_strangers_file() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    r.trash(f.id).await.unwrap();
    // the trash entry is gone (crash, cleanup), a stranger's file of the same size sits at the place
    std::fs::remove_file(root(&e).join(format!(".fs-files/trash/{}", f.id))).unwrap();
    std::fs::write(root(&e).join("a.txt"), b"other").unwrap();
    assert!(r.restore(f.id).await.is_err());
    assert!(r.get(f.id).await.unwrap().unwrap().trashed);
    assert_eq!(on_disk(&e, "a.txt"), b"other");
}
