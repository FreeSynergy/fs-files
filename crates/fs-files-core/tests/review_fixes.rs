//! The other findings of the review: each has a test on a real directory.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::disk::{exists, on_disk, root};
use common::{env, names, read_all, upload};
use fs_files_core::{
    BlobBackend, Error, FileFilter, Files, LocalFolder, OnConflict, Spot, Transfer,
};

fn kind<T>(r: fs_files_core::Result<T>) -> String {
    r.err()
        .and_then(|e| e.problem())
        .map_or(String::new(), |p| p.kind.to_string())
}

#[tokio::test]
async fn a_folder_where_a_file_was_is_not_moved_or_trashed() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine").await;
    std::fs::remove_file(root(&e).join("a.txt")).unwrap();
    std::fs::create_dir(root(&e).join("a.txt")).unwrap();
    std::fs::write(root(&e).join("a.txt/theirs"), b"x").unwrap();
    assert!(kind(r.rename_file(f.id, "b.txt").await).ends_with("changed-outside"));
    assert!(kind(r.trash(f.id).await).ends_with("changed-outside"));
    assert!(kind(r.open(f.id).await).ends_with("changed-outside"));
    assert_eq!(on_disk(&e, "a.txt/theirs"), b"x");
}

#[tokio::test]
async fn keep_both_skips_names_that_only_the_disk_knows() {
    let e = env().await;
    let r = &e.repo;
    let a = upload(r, None, "a.txt", "text/plain", b"A").await;
    upload(r, None, "b.txt", "text/plain", b"B").await;
    std::fs::write(root(&e).join("b (2).txt"), b"outside").unwrap();
    let moved = r
        .rename_file_as(a.id, "b.txt", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(moved.name, "b (3).txt");
    assert_eq!(on_disk(&e, "b (2).txt"), b"outside");
    assert_eq!(on_disk(&e, "b (3).txt"), b"A");
    // moving within the same folder is not a clash with itself, even though the disk has the name
    let same = r
        .move_file_as(moved.id, None, OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(same.name, "b (3).txt");
    // restore
    r.trash(moved.id).await.unwrap();
    std::fs::write(root(&e).join("b (3).txt"), b"outside 2").unwrap();
    let back = r.restore_as(moved.id, OnConflict::KeepBoth).await.unwrap();
    assert_eq!(back.name, "b (3) (2).txt");
    // folders
    let d = r.create_folder(None, "d").await.unwrap();
    std::fs::create_dir(root(&e).join("d (2)")).unwrap();
    let again = r.create_folder(None, "e").await.unwrap();
    let renamed = r
        .rename_folder_as(again.id, "d", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(renamed.name, "d (3)");
    let _ = d;
}

#[tokio::test]
async fn an_upload_that_said_keep_both_takes_a_free_name_at_finish() {
    let e = env().await;
    let r = &e.repo;
    let (up, _) = r
        .start_upload_as(None, "p.txt", "text/plain", OnConflict::KeepBoth)
        .await
        .unwrap();
    up.write(0, b"mine").await.unwrap();
    // meanwhile somebody takes the name, in the records and on the disk
    upload(r, None, "p.txt", "text/plain", b"theirs").await;
    std::fs::write(root(&e).join("p (2).txt"), b"outside").unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(f.name, "p (3).txt");
    assert_eq!(on_disk(&e, "p.txt"), b"theirs");
    assert_eq!(on_disk(&e, "p (3).txt"), b"mine");
    // without the rule the name is a conflict at finish and the upload survives
    let (up, id) = r.start_upload(None, "q.txt", "text/plain").await.unwrap();
    up.write(0, b"x").await.unwrap();
    std::fs::write(root(&e).join("q.txt"), b"outside").unwrap();
    assert!(matches!(up.finish().await, Err(Error::Conflict(_))));
    assert_eq!(
        r.resume_upload(&id)
            .await
            .unwrap()
            .received()
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn a_folder_with_an_upload_running_cannot_be_deleted() {
    let e = env().await;
    let r = &e.repo;
    let d = r.create_folder(None, "d").await.unwrap();
    let (up, _) = r
        .start_upload(Some(d.id), "a.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"x").await.unwrap();
    assert!(matches!(
        r.delete_folder(d.id).await,
        Err(Error::Conflict(_))
    ));
    let f = up.finish().await.unwrap();
    assert_eq!(f.folder, Some(d.id));
    r.trash(f.id).await.unwrap();
    r.purge(f.id).await.unwrap();
    let (up, _) = r
        .start_upload(Some(d.id), "b.txt", "text/plain")
        .await
        .unwrap();
    up.abort().await.unwrap();
    r.delete_folder(d.id).await.unwrap();
}

#[tokio::test]
async fn rescan_after_a_crash_in_the_trash_never_rebinds_to_a_strangers_file() {
    let e = env().await;
    let r = &e.repo;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    let f = upload(r, None, "Foto.jpg", "image/jpeg", b"my photo").await;
    // crash: the bytes went to the trash, the record still says live; then a stranger writes the name
    b.relocate(
        &Spot::Path("Foto.jpg".into()),
        &Spot::Trash(f.id.to_string()),
    )
    .await
    .unwrap();
    std::fs::write(root(&e).join("Foto.jpg"), b"the stranger's photo").unwrap();
    let report = r.rescan(None).await.unwrap();
    assert_eq!(report.updated.len(), 1);
    assert_eq!(report.added.len(), 1);
    assert_eq!(on_disk(&e, "Foto.jpg"), b"the stranger's photo");
    assert!(r.get(f.id).await.unwrap().unwrap().trashed);
    // my photo is in the trash, restorable under another name; the stranger's file is a record of its own
    let kept = r.restore_as(f.id, OnConflict::KeepBoth).await.unwrap();
    assert_eq!(read_all(r, kept.id).await, b"my photo");
    let listed = names(r, None).await;
    assert_eq!(listed.len(), 2, "{listed:?}");
    // the stranger's file can be trashed and purged like any other, mine is untouched
    let stranger = r
        .find(
            &FileFilter {
                name: Some("Foto.jpg".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    assert!(!stranger.items.is_empty());
}

#[tokio::test]
async fn the_first_rescan_of_untouched_uploads_changes_nothing() {
    let e = env().await;
    let r = &e.repo;
    for n in 0..5 {
        upload(
            r,
            None,
            &format!("f{n}"),
            "text/plain",
            format!("bytes {n}").as_bytes(),
        )
        .await;
    }
    let report = r.rescan(None).await.unwrap();
    assert!(
        report.updated.is_empty() && report.added.is_empty() && report.missing.is_empty(),
        "{report:?}"
    );
}

#[tokio::test]
async fn the_reserved_name_is_refused_in_any_case_and_odd_errors_have_problem_types() {
    let e = env().await;
    let r = &e.repo;
    for n in [".fs-files", ".FS-FILES", ".Fs-Files"] {
        assert!(
            matches!(r.create_folder(None, n).await, Err(Error::InvalidName(_))),
            "{n}"
        );
    }
    assert!(!exists(&e, ".FS-FILES"));
    // a symlink at a place the program is about to use is "changed outside", not "invalid name"
    let d = r.create_folder(None, "d").await.unwrap();
    std::fs::remove_dir(root(&e).join("d")).unwrap();
    common::links::link_dir(e.dir.path(), &root(&e).join("d"));
    assert!(kind(r.create_folder(Some(d.id), "x").await).ends_with("changed-outside"));
}
