//! Backends are swappable at runtime; files remember where their bytes are.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::{env, read_all, upload};
use fs_files_core::{Backends, Error, Files, Transfer};

fn local(id: &str, path: &std::path::Path, active: bool) -> String {
    serde_json::json!({ "id": id, "kind": "local", "path": path, "active": active }).to_string()
}

#[tokio::test]
async fn adding_a_backend_keeps_old_files_readable() {
    let env = env().await;
    let r = &env.repo;
    let old = upload(r, None, "old.txt", "text/plain", b"old").await;
    let second = env.dir.path().join("second");
    let info = r
        .add_backend(&local("second", &second, true))
        .await
        .unwrap();
    assert_eq!(
        (info.id.as_str(), info.kind.as_str(), info.active),
        ("second", "local", true)
    );
    let list = r.list_backends().await.unwrap();
    assert_eq!(
        list.iter()
            .map(|b| (b.id.as_str(), b.active))
            .collect::<Vec<_>>(),
        [("main", false), ("second", true)]
    );

    let new = upload(r, None, "new.txt", "text/plain", b"new").await;
    assert_eq!(r.backend_of(old.id).await.unwrap(), "main");
    assert_eq!(r.backend_of(new.id).await.unwrap(), "second");
    assert_eq!(read_all(r, old.id).await, b"old");
    assert_eq!(read_all(r, new.id).await, b"new");
    // copying stays on the backend of the source
    assert_eq!(
        r.backend_of(r.copy_file(old.id, None).await.unwrap().id)
            .await
            .unwrap(),
        "main"
    );
}

#[tokio::test]
async fn remove_is_refused_while_files_are_on_it() {
    let env = env().await;
    let r = &env.repo;
    let old = upload(r, None, "old.txt", "text/plain", b"old").await;
    r.add_backend(&local("second", &env.dir.path().join("second"), true))
        .await
        .unwrap();
    assert!(
        matches!(r.remove_backend("main").await, Err(Error::Conflict(_))),
        "has a file"
    );
    r.trash(old.id).await.unwrap();
    assert!(
        matches!(r.remove_backend("main").await, Err(Error::Conflict(_))),
        "a trashed file still counts"
    );
    r.purge(old.id).await.unwrap();
    r.remove_backend("main").await.unwrap();
    assert_eq!(r.list_backends().await.unwrap().len(), 1);
    assert!(matches!(
        r.remove_backend("main").await,
        Err(Error::NotFound(_))
    ));
    assert!(
        matches!(r.remove_backend("second").await, Err(Error::Conflict(_))),
        "the active one stays"
    );
}

#[tokio::test]
async fn a_running_upload_keeps_its_backend_from_being_removed() {
    let env = env().await;
    let r = &env.repo;
    let (up, _) = r.start_upload(None, "u", "text/plain").await.unwrap();
    r.add_backend(&local("second", &env.dir.path().join("second"), true))
        .await
        .unwrap();
    assert!(matches!(
        r.remove_backend("main").await,
        Err(Error::Conflict(_))
    ));
    up.write(0, b"x").await.unwrap();
    assert_eq!(
        r.backend_of(up.finish().await.unwrap().id).await.unwrap(),
        "main"
    );
}

#[tokio::test]
async fn a_bad_configuration_changes_nothing() {
    let env = env().await;
    let r = &env.repo;
    let before = r.list_backends().await.unwrap();
    let blocker = env.dir.path().join("blocker");
    std::fs::write(&blocker, b"a file, not a folder").unwrap();
    let bad = [
        "not json".to_string(),
        r#"{"kind":"local","path":"/tmp/x"}"#.to_string(),
        r#"{"id":"x"}"#.to_string(),
        r#"{"id":"Bad Id","kind":"local","path":"/tmp/x"}"#.to_string(),
        r#"{"id":"x","kind":"tape"}"#.to_string(),
        r#"{"id":"x","kind":"local"}"#.to_string(),
        local("x", &blocker.join("below"), true),
        local("main", &env.dir.path().join("again"), false), // id taken
    ];
    for cfg in bad {
        let err = r.add_backend(&cfg).await.unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::Io(_) | Error::Conflict(_)),
            "{cfg}: {err}"
        );
    }
    assert_eq!(r.list_backends().await.unwrap(), before);
    assert_eq!(
        r.list_backends()
            .await
            .unwrap()
            .iter()
            .find(|b| b.active)
            .unwrap()
            .id,
        "main"
    );
}

#[tokio::test]
async fn errors_never_repeat_the_configuration() {
    let env = env().await;
    let cfg = r#"{"id":"x","kind":"tape","secret_key":"hunter2-secret"}"#;
    let err = env.repo.add_backend(cfg).await.unwrap_err().to_string();
    assert!(!err.contains("hunter2"), "{err}");
    let err = env
        .repo
        .add_backend(r"hunter2-secret {")
        .await
        .unwrap_err()
        .to_string();
    assert!(!err.contains("hunter2"), "{err}");
}

#[tokio::test]
async fn the_swap_is_atomic_for_running_readers() {
    let env = env().await;
    let r = env.repo.clone();
    let f = upload(&r, None, "a.txt", "text/plain", b"abc").await;
    let reader = tokio::spawn({
        let r = r.clone();
        async move {
            for _ in 0..50 {
                assert_eq!(read_all(&r, f.id).await, b"abc");
                assert!(!r.list_backends().await.unwrap().is_empty());
            }
        }
    });
    for i in 0..5 {
        let cfg = local(
            &format!("b{i}"),
            &env.dir.path().join(format!("b{i}")),
            false,
        );
        r.add_backend(&cfg).await.unwrap();
    }
    reader.await.unwrap();
    assert_eq!(r.list_backends().await.unwrap().len(), 6);
}
