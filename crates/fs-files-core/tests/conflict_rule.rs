//! The rule for a taken name: refuse (default) or keep both. Nothing is ever replaced.

mod common;

use common::disk::{on_disk, root};
use common::{env, names, read_all, upload};
use fs_files_core::{Error, Files, OnConflict, Transfer};

#[tokio::test]
async fn refuse_is_the_default_and_changes_nothing() {
    let e = env().await;
    let r = &e.repo;
    let a = upload(r, None, "a.txt", "text/plain", b"A").await;
    upload(r, None, "b.txt", "text/plain", b"B").await;
    let d = r.create_folder(None, "d").await.unwrap();
    let other = upload(r, Some(d.id), "a.txt", "text/plain", b"other").await;

    for res in [
        r.rename_file_as(a.id, "b.txt", OnConflict::Refuse).await,
        r.move_file_as(a.id, Some(d.id), OnConflict::Refuse).await,
        r.copy_file_as(a.id, None, OnConflict::Refuse).await,
    ] {
        assert!(matches!(res, Err(Error::Conflict(_))), "{res:?}");
    }
    assert!(matches!(
        r.start_upload_as(None, "b.txt", "text/plain", OnConflict::Refuse)
            .await,
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        r.rename_folder_as(d.id, "a.txt", OnConflict::Refuse).await,
        Err(Error::Conflict(_))
    ));
    assert_eq!(names(r, None).await, ["a.txt", "b.txt", "d"]);
    assert_eq!(on_disk(&e, "a.txt"), b"A");
    assert_eq!(on_disk(&e, "d/a.txt"), b"other");
    assert_eq!(read_all(r, other.id).await, b"other");
}

#[tokio::test]
async fn keep_both_gives_the_incoming_file_a_free_name() {
    let e = env().await;
    let r = &e.repo;
    let a = upload(r, None, "a.txt", "text/plain", b"A").await;
    let b = upload(r, None, "b.txt", "text/plain", b"B").await;
    // rename
    let renamed = r
        .rename_file_as(a.id, "b.txt", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(renamed.name, "b (2).txt");
    assert_eq!(on_disk(&e, "b (2).txt"), b"A");
    assert_eq!(on_disk(&e, "b.txt"), b"B");
    // move into a folder that has the name
    let d = r.create_folder(None, "d").await.unwrap();
    upload(r, Some(d.id), "b.txt", "text/plain", b"in d").await;
    let moved = r
        .move_file_as(b.id, Some(d.id), OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(moved.name, "b (2).txt");
    assert_eq!(on_disk(&e, "d/b.txt"), b"in d");
    assert_eq!(on_disk(&e, "d/b (2).txt"), b"B");
    // moving within the same folder is not a clash with itself
    let same = r
        .move_file_as(moved.id, Some(d.id), OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(same.name, "b (2).txt");
    // copy: refuse by default, keep both on request
    assert!(matches!(
        r.copy_file_as(renamed.id, None, OnConflict::Refuse).await,
        Err(Error::Conflict(_))
    ));
    let copy = r
        .copy_file_as(renamed.id, None, OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(copy.name, "b (2) (copy).txt");
    assert_eq!(read_all(r, copy.id).await, b"A");
    // folders
    let f1 = r.create_folder(None, "f").await.unwrap();
    let f2 = r.create_folder(Some(d.id), "f").await.unwrap();
    let kept = r
        .move_folder_as(f2.id, None, OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(kept.name, "f (2)");
    assert!(root(&e).join("f").is_dir() && root(&e).join("f (2)").is_dir());
    assert!(!root(&e).join("d/f").exists());
    let again = r
        .rename_folder_as(kept.id, "f", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(again.name, "f (2)");
    let _ = f1;
}

#[tokio::test]
async fn keep_both_for_an_upload_looks_at_the_disk_too() {
    let e = env().await;
    let r = &e.repo;
    upload(r, None, "p.png", "image/png", b"one").await;
    std::fs::write(root(&e).join("p (2).png"), b"from outside").unwrap();
    let (up, _) = r
        .start_upload_as(None, "p.png", "image/png", OnConflict::KeepBoth)
        .await
        .unwrap();
    up.write(0, b"two").await.unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(f.name, "p (3).png");
    assert_eq!(on_disk(&e, "p.png"), b"one");
    assert_eq!(on_disk(&e, "p (2).png"), b"from outside");
    assert_eq!(on_disk(&e, "p (3).png"), b"two");
    // a name that is free stays as it is
    let (up, _) = r
        .start_upload_as(None, "free.txt", "text/plain", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(up.finish().await.unwrap().name, "free.txt");
}
