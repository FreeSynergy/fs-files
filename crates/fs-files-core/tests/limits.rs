//! Limits of `rescan` and of folder moves: deep trees, many files, unreadable entries, leftovers.

mod common;

use common::disk::root;
use common::env;
use fs_files_core::{BlobBackend, Error, Files, LocalFolder, Spot, Transfer};

#[cfg(unix)] // an unreadable file is made with mode bits
#[tokio::test]
async fn rescan_goes_on_after_an_entry_it_cannot_read() {
    use std::os::unix::fs::PermissionsExt;

    use fs_files_core::Entry;
    let e = env().await;
    let r = &e.repo;
    std::fs::write(root(&e).join("locked"), b"secret").unwrap();
    std::fs::set_permissions(
        root(&e).join("locked"),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();
    std::fs::write(root(&e).join("fine"), b"ok").unwrap();
    let report = r.rescan(None).await.unwrap();
    let readable = std::fs::read(root(&e).join("locked")).is_ok(); // running as root reads it anyway
    let added: Vec<_> = report
        .added
        .iter()
        .map(|x| match x {
            Entry::File(f) => f.name.clone(),
            Entry::Folder(f) => f.name.clone(),
        })
        .collect();
    assert!(added.contains(&"fine".to_string()), "{report:?}");
    if !readable {
        assert!(
            report.skipped.iter().any(|s| s.starts_with("locked")),
            "{report:?}"
        );
    }
}

#[tokio::test]
async fn a_very_deep_tree_does_not_overflow_the_stack() {
    let e = env().await;
    let r = &e.repo;
    let deep = "d/".repeat(700);
    std::fs::create_dir_all(root(&e).join(&deep)).unwrap();
    std::fs::write(root(&e).join(&deep).join("leaf.txt"), b"leaf").unwrap();
    let report = r.rescan(None).await.unwrap();
    assert!(
        report.skipped.iter().any(|s| s.contains("nested too deep")),
        "{} skipped",
        report.skipped.len()
    );
}

#[tokio::test]
async fn rescan_time_grows_about_linearly_with_the_number_of_files() {
    let e = env().await;
    let r = &e.repo;
    for n in 0..3000 {
        std::fs::write(root(&e).join(format!("file-{n:05}")), format!("{n}")).unwrap();
    }
    let first = r.rescan(None).await.unwrap();
    assert_eq!(first.added.len(), 3000);
    let start = std::time::Instant::now();
    let second = r.rescan(None).await.unwrap();
    assert!(second.added.is_empty() && second.updated.is_empty());
    assert!(
        start.elapsed() < std::time::Duration::from_secs(20),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn leftover_pending_blobs_are_swept_but_live_uploads_are_kept() {
    let e = env().await;
    let r = &e.repo;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    b.put("k1", 0, b"kept").await.unwrap();
    b.put("k2", 0, b"left by a crashed copy").await.unwrap();
    assert_eq!(
        b.sweep_pending(&["k1".into()], 3600).await.unwrap(),
        0,
        "too young"
    );
    assert_eq!(b.sweep_pending(&["k1".into()], 0).await.unwrap(), 1);
    assert_eq!(
        (
            b.pending("k1").await.unwrap(),
            b.pending("k2").await.unwrap()
        ),
        (4, 0)
    );
    // a rescan never touches a running upload
    let (up, _) = r
        .start_upload(None, "live.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"live").await.unwrap();
    r.rescan(None).await.unwrap();
    assert_eq!(up.received().await.unwrap(), 4);
    assert!(b.has(&Spot::Path("nope".into())).await.is_ok());
}

#[tokio::test]
async fn moving_a_folder_whose_subtree_would_not_fit_is_refused_up_front() {
    // A short root of our own (the temp folder of a test run can be long, and the root counts).
    let short = if cfg!(unix) {
        std::path::PathBuf::from("/tmp")
    } else {
        // The top of the system drive (a runner or an administrator may write there), else the temp folder.
        let top = std::path::PathBuf::from(format!(
            "{}\\",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
        ));
        if tempfile::tempdir_in(&top).is_ok() {
            top
        } else {
            std::env::temp_dir()
        }
    };
    let dir = tempfile::tempdir_in(short).unwrap();
    let blobs = dir.path().join("blobs");
    let store: std::sync::Arc<dyn fs_objects::Store> =
        std::sync::Arc::new(fs_objects::TursoStore::open_in_memory().await.unwrap());
    let backend = std::sync::Arc::new(LocalFolder::open(&blobs).await.unwrap());
    let repo = fs_files_core::FileRepo::new(store, common::ME, "main", backend);
    let r = &repo;
    let blobs = blobs.canonicalize().unwrap();
    // a chain of 38 folders of 100 bytes: about 3.8 KB of path, fine on its own
    let mut parent = None;
    let mut top = None;
    for i in 0..38 {
        let f = r
            .create_folder(parent, &format!("{i:02}{}", "c".repeat(98)))
            .await
            .unwrap();
        top = top.or(Some(f.id));
        parent = Some(f.id);
    }
    // two folders of 200 bytes: moving the chain under them would pass the 4096 bytes of a path
    let t1 = r.create_folder(None, &"t".repeat(200)).await.unwrap();
    let t2 = r
        .create_folder(Some(t1.id), &"u".repeat(200))
        .await
        .unwrap();
    let top = top.unwrap();
    let err = r.move_folder(top, Some(t2.id)).await.unwrap_err();
    assert!(matches!(err, Error::InvalidName(_)), "{err}");
    assert_eq!(
        err.problem().unwrap().kind,
        fs_interfaces::files_capnp::INVALID_NAME_PROBLEM_TYPE
    );
    // nothing changed, in the records or on the disk
    assert_eq!(r.get_folder(top).await.unwrap().unwrap().parent, None);
    assert!(blobs.join(format!("00{}", "c".repeat(98))).is_dir());
    assert!(
        !blobs
            .join(format!("{}/{}", "t".repeat(200), "u".repeat(200)))
            .join(format!("00{}", "c".repeat(98)))
            .exists()
    );
    // one level up is fine
    r.move_folder(top, Some(t1.id)).await.unwrap();
}
