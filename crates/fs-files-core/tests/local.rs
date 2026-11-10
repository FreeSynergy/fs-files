//! The `LocalFolder` backend: the shared suite plus what only a folder on disk can get wrong.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{stream, suite};
use fs_files_core::{BlobBackend, Error, LocalFolder};

async fn open(root: &Path) -> Arc<dyn BlobBackend> {
    Arc::new(LocalFolder::open(root).await.unwrap())
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[tokio::test]
async fn passes_the_shared_suite() {
    let dir = tmp();
    suite::run_all(&*open(dir.path()).await, "suite").await;
}

#[tokio::test]
async fn a_large_file_streams_in_chunks() {
    let dir = tmp();
    // 5.5 MiB in 300 KB chunks: bigger than the 1 MiB streaming chunk and not a multiple of it.
    stream::large(&*open(dir.path()).await, "big/file", 5_767_168, 300_000).await;
}

#[tokio::test]
async fn an_upload_resumes_after_a_restart() {
    let dir = tmp();
    let root = dir.path().to_path_buf();
    stream::resume(|| open(&root), "up/load", 3_000_000, 100_000).await;
}

#[tokio::test]
async fn files_are_ordinary_files_in_the_root() {
    let dir = tmp();
    let b = open(dir.path()).await;
    b.put("ab/c1d2", 0, b"plain").await.unwrap();
    b.commit("ab/c1d2").await.unwrap();
    let on_disk = std::fs::read(dir.path().join("ab").join("c1d2")).unwrap();
    assert_eq!(on_disk, b"plain");
    assert!(
        std::fs::read_dir(dir.path().join(".fs-files/pending"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn symlinks_never_lead_out_of_the_root() {
    let dir = tmp();
    let outside = tmp();
    std::fs::write(outside.path().join("secret"), b"top secret").unwrap();
    let root = dir.path().join("root");
    let b = open(&root).await;
    // a directory link, a file link, and a link as the pending folder
    common::links::link_dir(outside.path(), &root.join("dirlink"));
    common::links::link_file(&outside.path().join("secret"), &root.join("filelink"));
    for key in ["dirlink/secret", "filelink"] {
        assert!(
            matches!(b.get(key, 0, 99).await, Err(Error::InvalidKey(_))),
            "{key}"
        );
        assert!(
            matches!(b.stat(key).await, Err(Error::InvalidKey(_))),
            "{key}"
        );
        assert!(
            b.put(key, 0, b"x").await.is_ok(),
            "pending is flat and safe"
        );
        assert!(
            matches!(b.commit(key).await, Err(Error::InvalidKey(_))),
            "{key}"
        );
        assert!(
            matches!(b.delete(key).await, Err(Error::InvalidKey(_))),
            "{key}"
        );
    }
    assert_eq!(
        std::fs::read(outside.path().join("secret")).unwrap(),
        b"top secret"
    );
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        1,
        "nothing was created outside"
    );
}

#[tokio::test]
async fn a_symlinked_pending_folder_is_refused() {
    let dir = tmp();
    let outside = tmp();
    let root = dir.path().join("root");
    let b = open(&root).await;
    std::fs::remove_dir(root.join(".fs-files/pending")).unwrap();
    common::links::link_dir(outside.path(), &root.join(".fs-files/pending"));
    assert!(matches!(
        b.put("k", 0, b"x").await,
        Err(Error::InvalidKey(_))
    ));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn hostile_keys_create_nothing_anywhere() {
    let dir = tmp();
    let root = dir.path().join("root");
    let b = open(&root).await;
    for k in ["../x", "/tmp/fs-files-escape", "a/../../x"] {
        assert!(b.put(k, 0, b"x").await.is_err());
    }
    assert!(!dir.path().join("x").exists());
    assert!(!Path::new("/tmp/fs-files-escape").exists());
    assert!(!root.join("x").exists());
}
