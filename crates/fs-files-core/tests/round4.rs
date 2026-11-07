//! Fourth review: stale sessions in the list, local keys that are not probed, forged times.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use common::disk::root;
use common::flaky::Flaky;
use common::{ME, env, upload};
use fs_files_core::{BlobBackend, FileRepo, Files, LocalFolder, Transfer};
use fs_objects::{Store, TursoStore};

async fn flaky_repo(dir: &std::path::Path, page: usize) -> (FileRepo, Arc<Flaky>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let flaky = Arc::new(Flaky {
        store,
        fail: Arc::default(),
        fail_delete: Arc::default(),
    });
    let backend = Arc::new(LocalFolder::open(dir.join("blobs")).await.unwrap());
    let repo = FileRepo::with_backend(flaky.clone(), flaky.clone(), ME, "main", backend)
        .with_page_size(page);
    (repo, flaky)
}

async fn stale_sessions(r: &FileRepo, f: &Flaky, n: usize) {
    for i in 0..n {
        let (up, _) = r
            .start_upload(None, &format!("stale{i}"), "text/plain")
            .await
            .unwrap();
        up.write(0, b"x").await.unwrap();
        f.fail_delete.store(true, Ordering::SeqCst);
        let _ = up.finish().await; // the file is written, deleting the session fails
        f.fail_delete.store(false, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn stale_sessions_never_make_the_list_hang_or_repeat() {
    for (stale, page) in [(1, 100), (1, 2), (2, 2), (5, 2), (3, 1)] {
        let dir = tempfile::tempdir().unwrap();
        let (r, f) = flaky_repo(dir.path(), page).await;
        let (live_a, id_a) = r.start_upload(None, "live-a", "text/plain").await.unwrap();
        stale_sessions(&r, &f, stale).await;
        let (_b, id_b) = r.start_upload(None, "live-b", "text/plain").await.unwrap();
        let listed = tokio::time::timeout(Duration::from_secs(10), r.pending_uploads())
            .await
            .unwrap_or_else(|_| panic!("stale {stale}, page {page}: the list hangs"))
            .unwrap();
        let ids: Vec<_> = listed.iter().map(|u| u.id.clone()).collect();
        assert_eq!(ids, [id_a, id_b], "stale {stale}, page {page}");
        drop(live_a);
    }
}

#[tokio::test]
async fn files_named_like_shard_folders_do_not_break_uploads() {
    let e = env().await;
    let r = &e.repo;
    // people's files at the top with every possible two-hex-digit name
    for i in 0..256 {
        std::fs::write(root(&e).join(format!("{i:02x}")), b"a person's file").unwrap();
    }
    for n in 0..4 {
        let (up, _) = r
            .start_upload(None, &format!("up{n}.bin"), "application/octet-stream")
            .await
            .unwrap();
        assert_eq!(up.received().await.unwrap(), 0);
        up.write(0, b"bytes").await.unwrap();
        assert_eq!(up.received().await.unwrap(), 5);
        let f = up.finish().await.unwrap();
        assert_eq!(common::read_all(r, f.id).await, b"bytes");
    }
    assert_eq!(
        std::fs::read(root(&e).join("00")).unwrap(),
        b"a person's file"
    );
}

#[tokio::test]
async fn a_missing_or_epoch_modification_time_counts_as_changed() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    let path = root(&e).join("a.txt");
    std::fs::write(&path, b"other").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(std::time::UNIX_EPOCH)
        .unwrap();
    let err = r
        .open(f.id)
        .await
        .err()
        .expect("a stranger's bytes are not served");
    assert!(
        err.problem().unwrap().kind.ends_with("changed-outside"),
        "{err}"
    );
    assert!(r.trash(f.id).await.is_err());
}

#[tokio::test]
async fn the_orphan_limit_has_a_floor() {
    let e = env().await;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    b.put("not-ours", 0, b"young").await.unwrap();
    e.repo
        .clone()
        .with_orphan_after(Duration::ZERO)
        .rescan(None)
        .await
        .unwrap();
    assert_eq!(
        b.pending("not-ours").await.unwrap(),
        5,
        "a young blob is never removed right away"
    );
}

#[tokio::test]
async fn a_stray_file_in_the_trash_spot_is_never_moved_or_deleted_for_a_record() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    r.trash(f.id).await.unwrap();
    let spot = root(&e).join(format!(".fs-files/trash/{}", f.id));
    std::fs::write(&spot, b"stray bytes of some other size").unwrap();
    for res in [r.restore(f.id).await.map(|_| ()), r.purge(f.id).await] {
        let err = res.unwrap_err();
        assert!(
            err.problem().unwrap().kind.ends_with("changed-outside"),
            "{err}"
        );
    }
    assert_eq!(
        std::fs::read(&spot).unwrap(),
        b"stray bytes of some other size"
    );
    assert!(r.get(f.id).await.unwrap().unwrap().trashed);
}

#[tokio::test]
async fn rescan_with_a_stray_file_in_the_trash_spot_keeps_the_live_record() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    let spot = root(&e).join(format!(".fs-files/trash/{}", f.id));
    std::fs::write(&spot, b"stray").unwrap();
    let report = r.rescan(None).await.unwrap();
    assert!(
        report.added.is_empty() && report.missing.is_empty(),
        "{report:?}"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.contains("not the recorded file")),
        "{report:?}"
    );
    assert!(!r.get(f.id).await.unwrap().unwrap().trashed);
    assert_eq!(common::read_all(r, f.id).await, b"mine!");
    assert_eq!(std::fs::read(&spot).unwrap(), b"stray");
}
