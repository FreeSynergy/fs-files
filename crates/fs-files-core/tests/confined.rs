//! `backend.add` for a local folder stays inside the base the program was given: a path from outside
//! is refused as a bad configuration, and nothing is created or mounted.

mod common;

use common::{ME, env};
use fs_files_core::{Backends, Error, FileRepo, Files, LocalFolder};
use fs_store::{Store, TursoStore};
use std::sync::Arc;

fn local(id: &str, path: &str) -> String {
    serde_json::json!({ "id": id, "kind": "local", "path": path, "active": false }).to_string()
}

async fn confined(base: &std::path::Path) -> FileRepo {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let backend = Arc::new(LocalFolder::open(base).await.unwrap());
    FileRepo::new(store, ME, "main", backend).with_local_base(base)
}

#[tokio::test]
async fn a_path_below_the_base_is_made_there_and_every_other_is_refused() {
    let e = env().await;
    let (base, outside) = (e.dir.path().join("base"), e.dir.path().join("outside"));
    std::fs::create_dir_all(&outside).unwrap();
    let r = confined(&base).await;
    std::os::unix::fs::symlink(&outside, base.join("link")).unwrap();

    let info = r
        .add_backend(&local("second", "second/inner"))
        .await
        .unwrap();
    assert_eq!(info.id, "second");
    assert!(base.join("second/inner").is_dir());

    let outside_text = outside.to_str().unwrap();
    for bad in [
        outside_text,
        "/etc",
        "../outside",
        "second/../../outside",
        "link",
        "link/deeper",
        "",
        ".",
        "second/..",
        ".fs-files",
        ".FS-Files",
        "second/.fs-files",
    ] {
        let err = r.add_backend(&local("bad", bad)).await.unwrap_err();
        assert!(matches!(err, Error::Config(_)), "{bad:?}: {err}");
    }
    assert!(!outside.join("deeper").exists(), "nothing was made outside");
    let ids: Vec<_> = r
        .list_backends()
        .await
        .unwrap()
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(ids, ["main", "second"], "nothing else was mounted");
}

#[tokio::test]
async fn a_backend_added_below_the_base_is_no_part_of_the_main_one() {
    let e = env().await;
    let root = e.dir.path().join("root");
    let base = root.join(".fs-files/backends");
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let main = Arc::new(LocalFolder::open(&root).await.unwrap());
    std::fs::create_dir_all(&base).unwrap();
    let r = FileRepo::new(store, ME, "main", main).with_local_base(&base);
    std::fs::create_dir(root.join("Docs")).unwrap();
    std::fs::write(root.join("Docs/mine.txt"), "main's").unwrap();

    let cfg = |id: &str, path: &str, active: bool| {
        serde_json::json!({ "id": id, "kind": "local", "path": path, "active": active }).to_string()
    };
    r.add_backend(&cfg("second", "Docs", true)).await.unwrap();
    assert_eq!(
        std::fs::read(root.join("Docs/mine.txt")).unwrap(),
        b"main's"
    );
    assert!(base.join("Docs").is_dir() && !base.join("Docs/mine.txt").exists());
    common::upload(&r, None, "other.txt", "text/plain", b"second's").await;
    assert!(base.join("Docs/other.txt").is_file());

    let report = r.rescan(None).await.unwrap();
    let added: Vec<_> = report.added.iter().map(|x| format!("{x:?}")).collect();
    assert!(!added.iter().any(|a| a.contains("other.txt")), "{added:?}");
    assert!(
        !report.skipped.iter().any(|s| s.contains("backends")),
        "{:?}",
        report.skipped
    );

    // A backend inside another backend would be part of it.
    let err = r
        .add_backend(&cfg("nested", "Docs/inner", false))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");
}
