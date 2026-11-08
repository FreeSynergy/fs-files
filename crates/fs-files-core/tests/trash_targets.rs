//! Emptying the trash and choosing where a file is restored.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::disk::{exists, on_disk, root};
use common::{env, names, read_all, upload};
use fs_files_core::{Error, FileFilter, Files, OnConflict};

#[tokio::test]
async fn emptying_the_trash_purges_everything_in_it_and_nothing_else() {
    let e = env().await;
    let r = &e.repo;
    let d = r.create_folder(None, "d").await.unwrap();
    let keep = upload(r, None, "keep.txt", "text/plain", b"keep").await;
    for n in 0..5 {
        let f = upload(r, Some(d.id), &format!("t{n}.txt"), "text/plain", b"x").await;
        r.trash(f.id).await.unwrap();
    }
    let report = r.empty_trash().await.unwrap();
    assert_eq!((report.purged, report.failed.len()), (5, 0));
    let trash = FileFilter {
        trashed: true,
        ..Default::default()
    };
    assert!(r.find(&trash, None).await.unwrap().items.is_empty());
    assert!(
        std::fs::read_dir(root(&e).join(".fs-files/trash"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(read_all(r, keep.id).await, b"keep");
    r.delete_folder(d.id).await.unwrap(); // the trash no longer counts as content
    assert_eq!(r.empty_trash().await.unwrap().purged, 0);
}

#[tokio::test]
async fn a_file_can_be_restored_to_another_folder() {
    let e = env().await;
    let r = &e.repo;
    let from = r.create_folder(None, "from").await.unwrap();
    let to = r.create_folder(None, "to").await.unwrap();
    let f = upload(r, Some(from.id), "a.txt", "text/plain", b"bytes").await;
    r.trash(f.id).await.unwrap();

    let back = r
        .restore_to(f.id, Some(to.id), OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(back.folder, Some(to.id));
    assert_eq!(on_disk(&e, "to/a.txt"), b"bytes");
    assert!(!exists(&e, "from/a.txt"));
    assert_eq!(read_all(r, f.id).await, b"bytes");

    // to the top level
    r.trash(f.id).await.unwrap();
    let top = r.restore_to(f.id, None, OnConflict::Refuse).await.unwrap();
    assert_eq!(top.folder, None);
    assert_eq!(on_disk(&e, "a.txt"), b"bytes");
    assert_eq!(names(r, None).await, ["a.txt", "from", "to"]);

    // a folder that does not exist
    r.trash(f.id).await.unwrap();
    let nowhere = fs_objects::ObjectId::new();
    assert!(matches!(
        r.restore_to(f.id, Some(nowhere), OnConflict::Refuse).await,
        Err(Error::NotFound(_))
    ));
    assert!(
        root(&e).join(format!(".fs-files/trash/{}", f.id)).is_file(),
        "still in the trash"
    );
}

#[tokio::test]
async fn restoring_into_a_taken_name_refuses_or_keeps_both() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"old").await;
    r.trash(f.id).await.unwrap();
    upload(r, None, "a.txt", "text/plain", b"new").await;

    let err = r.restore_as(f.id, OnConflict::Refuse).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)));
    assert_eq!(on_disk(&e, "a.txt"), b"new");

    let kept = r.restore_as(f.id, OnConflict::KeepBoth).await.unwrap();
    assert_eq!(kept.name, "a (2).txt");
    assert_eq!(on_disk(&e, "a.txt"), b"new");
    assert_eq!(on_disk(&e, "a (2).txt"), b"old");

    // a name taken only on the disk is a conflict too, and the bytes stay in the trash
    r.trash(kept.id).await.unwrap();
    std::fs::write(root(&e).join("a (2).txt"), b"outside").unwrap();
    assert!(matches!(
        r.restore_as(kept.id, OnConflict::Refuse).await,
        Err(Error::Conflict(_))
    ));
    assert_eq!(on_disk(&e, "a (2).txt"), b"outside");
    assert!(
        root(&e)
            .join(format!(".fs-files/trash/{}", kept.id))
            .is_file()
    );
}
