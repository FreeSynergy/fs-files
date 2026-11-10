//! Folders: names, nesting, moving, deleting.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::{env, names, upload};
use fs_files_core::{Error, Files, Transfer};
use fs_objects::ObjectId;

#[tokio::test]
async fn folders_nest_and_list_in_name_order() {
    let e = env().await;
    let b = e.repo.create_folder(None, "Beta").await.unwrap();
    let a = e.repo.create_folder(None, "alpha").await.unwrap();
    let sub = e.repo.create_folder(Some(a.id), "inner").await.unwrap();
    assert_eq!(names(&e.repo, None).await, ["alpha", "Beta"]);
    assert_eq!(names(&e.repo, Some(a.id)).await, ["inner"]);
    assert_eq!(sub.parent, Some(a.id));
    assert_eq!(b.parent, None);
    assert!(sub.iri.ends_with(&format!("/files/{}", sub.id)));
    let renamed = e.repo.rename_folder(b.id, "Gamma").await.unwrap();
    assert_eq!(renamed.name, "Gamma");
    assert_eq!(
        e.repo.get_folder(b.id).await.unwrap().unwrap().name,
        "Gamma"
    );
}

#[tokio::test]
async fn names_are_unique_per_folder_exactly() {
    let e = env().await;
    let a = e.repo.create_folder(None, "Photos").await.unwrap();
    assert!(matches!(
        e.repo.create_folder(None, "Photos").await,
        Err(Error::Conflict(_))
    ));
    // the operating system decides: on Linux a different case is a different name, on Windows it is the same one
    let lower = e.repo.create_folder(None, "photos").await;
    if cfg!(windows) {
        assert!(matches!(lower, Err(Error::Conflict(_))));
    } else {
        lower.unwrap();
    }
    // the same name in another folder is fine, and a file and a folder share one namespace
    e.repo.create_folder(Some(a.id), "Photos").await.unwrap();
    upload(&e.repo, None, "x.txt", "text/plain", b"x").await;
    assert!(matches!(
        e.repo.create_folder(None, "x.txt").await,
        Err(Error::Conflict(_))
    ));
    let other = e.repo.create_folder(None, "Other").await.unwrap();
    assert!(matches!(
        e.repo.rename_folder(other.id, "Photos").await,
        Err(Error::Conflict(_))
    ));
    e.repo.rename_folder(other.id, "OTHER").await.unwrap();
}

#[tokio::test]
async fn hostile_and_empty_names_are_refused() {
    let e = env().await;
    for n in [
        "",
        "  ",
        ".",
        "..",
        "../x",
        "a/b",
        "a\\b",
        "a\0b",
        "tab\tname",
        &"n".repeat(256),
    ] {
        assert!(
            matches!(
                e.repo.create_folder(None, n).await,
                Err(Error::InvalidName(_))
            ),
            "{n:?}"
        );
        assert!(
            matches!(
                e.repo.start_upload(None, n, "text/plain").await,
                Err(Error::InvalidName(_))
            ),
            "{n:?}"
        );
    }
    assert!(names(&e.repo, None).await.is_empty());
}

#[tokio::test]
async fn a_folder_cannot_move_into_itself_or_below() {
    let e = env().await;
    let a = e.repo.create_folder(None, "a").await.unwrap();
    let b = e.repo.create_folder(Some(a.id), "b").await.unwrap();
    let c = e.repo.create_folder(Some(b.id), "c").await.unwrap();
    for target in [a.id, b.id, c.id] {
        let err = e.repo.move_folder(a.id, Some(target)).await.unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
    }
    // nothing changed; a legal move works
    assert_eq!(e.repo.get_folder(a.id).await.unwrap().unwrap().parent, None);
    let moved = e.repo.move_folder(c.id, None).await.unwrap();
    assert_eq!(moved.parent, None);
    e.repo.move_folder(a.id, Some(c.id)).await.unwrap();
    assert!(matches!(
        e.repo.move_folder(c.id, Some(b.id)).await,
        Err(Error::Conflict(_))
    ));
}

#[tokio::test]
async fn only_an_empty_folder_can_be_deleted() {
    let e = env().await;
    let a = e.repo.create_folder(None, "a").await.unwrap();
    let sub = e.repo.create_folder(Some(a.id), "sub").await.unwrap();
    assert!(
        matches!(e.repo.delete_folder(a.id).await, Err(Error::Conflict(_))),
        "subfolder"
    );
    e.repo.delete_folder(sub.id).await.unwrap();
    let f = upload(&e.repo, Some(a.id), "f.txt", "text/plain", b"hi").await;
    assert!(
        matches!(e.repo.delete_folder(a.id).await, Err(Error::Conflict(_))),
        "live file"
    );
    e.repo.trash(f.id).await.unwrap();
    assert!(
        matches!(e.repo.delete_folder(a.id).await, Err(Error::Conflict(_))),
        "trashed file counts"
    );
    e.repo.purge(f.id).await.unwrap();
    e.repo.delete_folder(a.id).await.unwrap();
    assert!(e.repo.get_folder(a.id).await.unwrap().is_none());
    assert!(matches!(
        e.repo.delete_folder(a.id).await,
        Err(Error::NotFound(_))
    ));
}

#[tokio::test]
async fn parents_must_exist() {
    let e = env().await;
    let ghost = Some(ObjectId::new());
    assert!(matches!(
        e.repo.create_folder(ghost, "x").await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        e.repo.start_upload(ghost, "x", "text/plain").await,
        Err(Error::NotFound(_))
    ));
}
