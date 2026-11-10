//! The `Backend` of `Picture`, `Album` and `StorageBackend`, and what is refused as unsupported.
//!
//! Pattern: Integration test — as `backend.rs`.

mod support;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{
    AlbumBackend, FileBackend, FolderBackend, PictureBackend, StorageBackendBackend,
};
use support::{url, v, world};

#[test]
fn pictures_are_the_image_files_and_albums_do_not_exist_yet() {
    let w = world();
    let (files, pictures) = (
        FileBackend(w.remote.clone()),
        PictureBackend(w.remote.clone()),
    );
    let (photo, text) = (w.dir.join("p.png"), w.dir.join("t.txt"));
    std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap();
    std::fs::write(&text, "x").unwrap();
    let id = files
        .run(
            "upload",
            &v(&[("source", &url(&photo)), ("mediaType", "image/png")]),
        )
        .unwrap()
        .id
        .unwrap();
    files.run("upload", &v(&[("source", &url(&text))])).unwrap();
    let rows = pictures.list(&Values::new()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values["file"], std::slice::from_ref(&id));

    pictures
        .run(
            "sensitiveMark",
            &v(&[("picture", &id), ("sensitive", "dpv:Health")]),
        )
        .unwrap();
    assert_eq!(
        pictures.list(&Values::new()).unwrap()[0].values["sensitive"],
        ["dpv:Health"]
    );
    let target = w.dir.join("copy.png");
    pictures
        .run(
            "download",
            &v(&[("picture", &id), ("target", &url(&target))]),
        )
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap().len(), 4);

    assert!(AlbumBackend.list(&Values::new()).unwrap().is_empty());
}

#[test]
fn what_the_core_lacks_is_refused_with_the_name_and_never_faked() {
    let w = world();
    let unsupported = |e: Result<_, BackendError>, name: &str| match e {
        Err(BackendError::Unavailable(t)) => assert_eq!(t, format!("unsupported:{name}")),
        other => panic!("{name}: {other:?}"),
    };
    unsupported(
        FileBackend(w.remote.clone()).run("preview", &Values::new()),
        "preview",
    );
    unsupported(
        FileBackend(w.remote.clone()).run("shareCreate", &Values::new()),
        "shareCreate",
    );
    unsupported(
        FolderBackend(w.remote.clone()).run("archive", &Values::new()),
        "archive",
    );
    unsupported(
        PictureBackend(w.remote.clone()).run("tagAdd", &Values::new()),
        "tagAdd",
    );
    unsupported(
        StorageBackendBackend(w.remote.clone()).run("quota", &Values::new()),
        "quota",
    );
    unsupported(AlbumBackend.run("create", &Values::new()), "create");
}

#[test]
fn storage_backends_are_listed_and_a_bad_configuration_is_refused() {
    let w = world();
    let storage = StorageBackendBackend(w.remote.clone());
    let rows = storage.list(&Values::new()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "main");
    assert_eq!(rows[0].values["state"], ["active"]);
    assert!(
        storage
            .run("create", &v(&[("config", "{\"kind\":\"nope\"}")]))
            .is_err()
    );
    let second = serde_json_config("second");
    storage.run("create", &v(&[("config", &second)])).unwrap();
    assert_eq!(storage.list(&Values::new()).unwrap().len(), 2);
    storage.run("delete", &v(&[("storage", "second")])).unwrap();
    assert_eq!(storage.list(&Values::new()).unwrap().len(), 1);
}

fn serde_json_config(id: &str) -> String {
    format!("{{\"id\":\"{id}\",\"kind\":\"local\",\"path\":\"{id}\",\"active\":false}}")
}

#[test]
fn unfinished_uploads_are_listed_and_can_be_aborted() {
    let w = world();
    let files = FileBackend(w.remote.clone());
    let none = files.run("uploadList", &Values::new()).unwrap();
    assert!(none.values.is_empty());
    assert!(
        files
            .run("uploadAbort", &v(&[("uploadId", "nothing")]))
            .is_err()
    );
}
