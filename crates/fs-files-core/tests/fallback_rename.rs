//! The no-replace move without the kernel's `RENAME_NOREPLACE`: files by hard link + unlink,
//! folders by look-then-rename. It never runs on a modern Linux, so the tests switch it on.

mod common;

use std::sync::Arc;

use common::{ME, read_all};
use fs_files_core::{BlobBackend, Error, FileRepo, Files, LocalFolder, Spot, Transfer};
use fs_objects::{Store, TursoStore};

async fn backend(dir: &std::path::Path) -> LocalFolder {
    LocalFolder::open(dir.join("root"))
        .await
        .unwrap()
        .without_native_rename()
}

#[tokio::test]
async fn files_are_moved_without_replacing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let b = backend(dir.path()).await;
    let root = b.root().to_path_buf();
    b.put("up1", 0, b"first").await.unwrap();
    b.publish("up1", "a/one.txt").await.unwrap();
    assert_eq!(std::fs::read(root.join("a/one.txt")).unwrap(), b"first");
    assert!(
        std::fs::read_dir(b.pending_dir()).unwrap().next().is_none(),
        "the pending name is gone"
    );
    // publishing onto a taken name: Conflict, the file stays, the upload stays pending
    b.put("up2", 0, b"second").await.unwrap();
    assert!(matches!(
        b.publish("up2", "a/one.txt").await,
        Err(Error::Conflict(_))
    ));
    assert_eq!(std::fs::read(root.join("a/one.txt")).unwrap(), b"first");
    assert_eq!(b.pending("up2").await.unwrap(), 6);
    // commit never replaces either
    assert!(b.commit("up2").await.is_ok());
    b.put("up2", 0, b"third").await.unwrap();
    assert!(matches!(b.commit("up2").await, Err(Error::Conflict(_))));
    // relocate
    b.relocate(
        &Spot::Path("a/one.txt".into()),
        &Spot::Path("b/two.txt".into()),
    )
    .await
    .unwrap();
    assert!(!root.join("a/one.txt").exists());
    assert_eq!(std::fs::read(root.join("b/two.txt")).unwrap(), b"first");
    std::fs::write(root.join("b/taken"), b"theirs").unwrap();
    let err = b
        .relocate(
            &Spot::Path("b/two.txt".into()),
            &Spot::Path("b/taken".into()),
        )
        .await;
    assert!(matches!(err, Err(Error::Conflict(_))));
    assert_eq!(std::fs::read(root.join("b/taken")).unwrap(), b"theirs");
    assert_eq!(std::fs::read(root.join("b/two.txt")).unwrap(), b"first");
    // onto a folder
    std::fs::create_dir(root.join("b/dir")).unwrap();
    assert!(matches!(
        b.relocate(&Spot::Path("b/two.txt".into()), &Spot::Path("b/dir".into()))
            .await,
        Err(Error::Conflict(_))
    ));
}

#[tokio::test]
async fn folders_are_moved_when_the_target_is_free_and_refused_when_it_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let b = backend(dir.path()).await;
    let root = b.root().to_path_buf();
    b.make_dir("x/y", true).await.unwrap();
    std::fs::write(root.join("x/y/f"), b"in").unwrap();
    b.relocate(&Spot::Path("x".into()), &Spot::Path("z".into()))
        .await
        .unwrap();
    assert_eq!(std::fs::read(root.join("z/y/f")).unwrap(), b"in");
    std::fs::create_dir_all(root.join("w/keep")).unwrap();
    std::fs::write(root.join("w/keep/theirs"), b"t").unwrap();
    assert!(matches!(
        b.relocate(&Spot::Path("z".into()), &Spot::Path("w".into()))
            .await,
        Err(Error::Conflict(_))
    ));
    assert!(root.join("z/y/f").exists() && root.join("w/keep/theirs").exists());
}

#[tokio::test]
async fn the_whole_lifecycle_works_on_the_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let r = FileRepo::new(store, ME, "main", Arc::new(backend(dir.path()).await));
    let d = r.create_folder(None, "d").await.unwrap();
    let (up, _) = r
        .start_upload(Some(d.id), "a.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"bytes").await.unwrap();
    let f = up.finish().await.unwrap();
    let f = r.rename_file(f.id, "b.txt").await.unwrap();
    r.trash(f.id).await.unwrap();
    r.restore(f.id).await.unwrap();
    r.rename_folder(d.id, "e").await.unwrap();
    assert_eq!(read_all(&r, f.id).await, b"bytes");
    assert_eq!(
        std::fs::read(dir.path().join("root/e/b.txt")).unwrap(),
        b"bytes"
    );
}
