//! Nothing the program does ever replaces a file or folder that is already there.
#![allow(clippy::assert_is_empty, clippy::many_single_char_names)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons; tests: short bindings (a, b, x, y) are the clear names here

mod common;

use common::disk::{is_conflict, on_disk, root};
use common::{env, names, read_all, upload};
use fs_files_core::{Error, Files, Transfer};

#[tokio::test]
async fn nothing_is_ever_overwritten() {
    let e = env().await;
    let r = &e.repo;
    let a = upload(r, None, "a.txt", "text/plain", b"mine").await;
    // somebody puts files and a folder in the way from outside
    std::fs::write(root(&e).join("taken.txt"), b"theirs").unwrap();
    std::fs::create_dir(root(&e).join("taken-dir")).unwrap();

    assert!(is_conflict(r.rename_file(a.id, "taken.txt").await));
    assert!(is_conflict(r.create_folder(None, "taken-dir").await));
    assert!(is_conflict(r.create_folder(None, "taken.txt").await));
    assert!(is_conflict(
        r.start_upload(None, "taken.txt", "text/plain")
            .await
            .map(|_| ())
    ));
    assert_eq!(on_disk(&e, "taken.txt"), b"theirs");
    assert_eq!(on_disk(&e, "a.txt"), b"mine");
    assert_eq!(names(r, None).await, ["a.txt"]);

    // a file appears at the target *after* the upload started: finish is a Conflict, the upload survives
    let (up, id) = r
        .start_upload(None, "late.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"upload").await.unwrap();
    std::fs::write(root(&e).join("late.txt"), b"theirs").unwrap();
    assert!(is_conflict(up.finish().await));
    assert_eq!(on_disk(&e, "late.txt"), b"theirs");
    assert_eq!(
        r.resume_upload(&id)
            .await
            .unwrap()
            .received()
            .await
            .unwrap(),
        6
    );
    std::fs::remove_file(root(&e).join("late.txt")).unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(read_all(r, f.id).await, b"upload");

    // restore into a name taken from outside is a Conflict and keeps the bytes in the trash
    r.trash(a.id).await.unwrap();
    std::fs::write(root(&e).join("a.txt"), b"newcomer").unwrap();
    assert!(is_conflict(r.restore(a.id).await));
    assert_eq!(on_disk(&e, "a.txt"), b"newcomer");
    assert!(root(&e).join(format!(".fs-files/trash/{}", a.id)).is_file());
    std::fs::remove_file(root(&e).join("a.txt")).unwrap();
    r.restore(a.id).await.unwrap();
    assert_eq!(read_all(r, a.id).await, b"mine");

    // folder rename and move onto something that is there
    let d = r.create_folder(None, "d").await.unwrap();
    std::fs::create_dir(root(&e).join("e")).unwrap();
    std::fs::write(root(&e).join("e/inside"), b"x").unwrap();
    assert!(is_conflict(r.rename_folder(d.id, "e").await));
    assert!(root(&e).join("d").is_dir());
    assert_eq!(r.get_folder(d.id).await.unwrap().unwrap().name, "d");
    // a copy never takes a name that is on the disk either
    let c = r.copy_file(a.id, None).await.unwrap();
    assert_eq!(c.name, "a (copy).txt");
}

#[tokio::test]
async fn the_programs_folder_is_not_a_user_name_and_is_not_listed() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"x").await;
    r.trash(f.id).await.unwrap();
    assert!(names(r, None).await.is_empty());
    for n in [".fs-files", ".fs-files "] {
        let res = r.create_folder(None, n).await;
        if n == ".fs-files" {
            assert!(matches!(res, Err(Error::InvalidName(_))));
        }
    }
    let report = r.rescan(None).await.unwrap();
    assert!(
        report.added.is_empty() && report.skipped.is_empty(),
        "{report:?}"
    );
    assert_eq!(
        r.find(
            &fs_files_core::FileFilter {
                trashed: true,
                ..Default::default()
            },
            None
        )
        .await
        .unwrap()
        .items
        .len(),
        1
    );
}
