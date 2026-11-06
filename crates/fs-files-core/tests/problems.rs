//! Errors map to the named problem types of the contract, and the envelope name is the real name.

mod common;

use common::{env, upload};
use fs_files_core::{Error, Files, Transfer};
use fs_interfaces::files_capnp::{
    CHANGED_OUTSIDE_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE,
};

#[tokio::test]
async fn a_name_clash_is_a_named_conflict() {
    let e = env().await;
    let a = upload(&e.repo, None, "a.txt", "text/plain", b"a").await;
    upload(&e.repo, None, "b.txt", "text/plain", b"b").await;
    let err = e.repo.rename_file(a.id, "b.txt").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)), "still a Conflict");
    let p = err.problem().expect("a name clash has a problem");
    assert_eq!((p.kind, p.status), (NAME_CONFLICT_PROBLEM_TYPE, 409));
    assert!(p.detail.contains("b.txt"));
    // a clash only the disk sees
    std::fs::write(e.dir.path().join("blobs/outside.txt"), b"x").unwrap();
    let err = e.repo.rename_file(a.id, "outside.txt").await.unwrap_err();
    assert_eq!(err.problem().unwrap().kind, NAME_CONFLICT_PROBLEM_TYPE);
    // other conflicts are not name conflicts
    let err = e.repo.purge(a.id).await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)));
    assert!(err.problem().is_none());
}

#[tokio::test]
async fn an_invalid_name_is_a_named_problem() {
    let e = env().await;
    for n in ["", "a/b", "..", ".fs-files", "x\0y"] {
        let err = e.repo.create_folder(None, n).await.unwrap_err();
        let p = err.problem().unwrap_or_else(|| panic!("{n:?}: {err}"));
        assert_eq!(
            (p.kind, p.status),
            (INVALID_NAME_PROBLEM_TYPE, 422),
            "{n:?}"
        );
    }
    let err = e
        .repo
        .start_upload(None, &"n".repeat(300), "text/plain")
        .await
        .err()
        .unwrap();
    assert_eq!(err.problem().unwrap().kind, INVALID_NAME_PROBLEM_TYPE);
}

#[tokio::test]
async fn a_file_changed_outside_is_a_named_problem() {
    let e = env().await;
    let f = upload(&e.repo, None, "a.txt", "text/plain", b"abc").await;
    std::fs::write(e.dir.path().join("blobs/a.txt"), b"abcdef").unwrap();
    let err = e.repo.open(f.id).await.err().unwrap();
    assert_eq!(err.problem().unwrap().kind, CHANGED_OUTSIDE_PROBLEM_TYPE);
    std::fs::remove_file(e.dir.path().join("blobs/a.txt")).unwrap();
    let err = e.repo.open(f.id).await.err().unwrap();
    assert_eq!(err.problem().unwrap().kind, CHANGED_OUTSIDE_PROBLEM_TYPE);
}

#[tokio::test]
async fn the_envelope_name_is_the_real_name() {
    let e = env().await;
    let d = e.repo.create_folder(None, "Urlaub").await.unwrap();
    let f = upload(&e.repo, Some(d.id), "Foto 1.jpg", "image/jpeg", b"x").await;
    for (id, name) in [(d.id, "Urlaub"), (f.id, "Foto 1.jpg")] {
        let o = e.store.get(id).await.unwrap().unwrap();
        assert_eq!(o.envelope.name, name);
    }
    let renamed = e.repo.rename_file(f.id, "Bild.jpg").await.unwrap();
    assert_eq!(
        e.store.get(f.id).await.unwrap().unwrap().envelope.name,
        "Bild.jpg"
    );
    assert_eq!(renamed.name, "Bild.jpg");
    assert!(e.dir.path().join("blobs/Urlaub/Bild.jpg").is_file());
}
