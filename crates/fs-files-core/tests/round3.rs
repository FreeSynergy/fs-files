//! Third review: checksums before destructive steps, quiet uploads, root length, copies, odd spots.

mod common;

use std::time::Duration;

use common::disk::{on_disk, root};
use common::{env, read_all, upload};
use fs_files_core::{BlobBackend, Error, Files, LocalFolder, OnConflict};

fn kind<T>(r: fs_files_core::Result<T>) -> String {
    r.err()
        .and_then(|e| e.problem())
        .map_or(String::new(), |p| p.kind.to_string())
}

/// Replace a file by other bytes of the same size and give it the old modification time (`touch -r`).
fn forge(path: &std::path::Path, bytes: &[u8]) {
    let mtime = std::fs::metadata(path).unwrap().modified().unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
}

#[tokio::test]
async fn a_forged_modification_time_does_not_hide_a_strangers_file() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    forge(&root(&e).join("a.txt"), b"other");
    assert!(kind(r.trash(f.id).await).ends_with("changed-outside"));
    assert!(kind(r.rename_file(f.id, "b.txt").await).ends_with("changed-outside"));
    assert_eq!(on_disk(&e, "a.txt"), b"other");
    // restore after a lost trash entry takes over nothing unverified
    let g = upload(r, None, "g.txt", "text/plain", b"mine!").await;
    r.trash(g.id).await.unwrap();
    std::fs::remove_file(root(&e).join(format!(".fs-files/trash/{}", g.id))).unwrap();
    std::fs::write(root(&e).join("g.txt"), b"other").unwrap();
    assert!(r.restore(g.id).await.is_err());
}

#[tokio::test]
async fn a_quiet_upload_survives_a_rescan_unless_the_limit_is_set_short() {
    let e = env().await;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    b.put("someone-elses/upload", 0, b"quiet").await.unwrap();
    e.repo.rescan(None).await.unwrap(); // default limit: 30 days
    assert_eq!(b.pending("someone-elses/upload").await.unwrap(), 5);
    let short = e.repo.clone().with_orphan_after(Duration::ZERO);
    short.rescan(None).await.unwrap();
    assert_eq!(b.pending("someone-elses/upload").await.unwrap(), 0);
}

#[tokio::test]
async fn a_root_that_leaves_little_room_gives_invalid_name_not_io() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    while deep.as_os_str().len() < 3900 {
        deep.push("d".repeat(200));
    }
    std::fs::create_dir_all(&deep).unwrap();
    let b = LocalFolder::open(&deep).await.unwrap();
    let key = format!("{}/{}", "k".repeat(100), "l".repeat(100));
    let err = b.make_dir(&key, true).await.unwrap_err();
    assert!(matches!(err, Error::InvalidKey(_)), "{err}");
    assert!(err.problem().unwrap().kind.ends_with("invalid-name"));
    assert!(b.make_dir("short", true).await.is_ok());
}

#[tokio::test]
async fn a_folder_move_counts_the_root_of_the_disk_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().join("blobs");
    while deep.as_os_str().len() < 3600 {
        deep.push("d".repeat(200));
    }
    std::fs::create_dir_all(&deep).unwrap();
    let store: std::sync::Arc<dyn fs_store::Store> =
        std::sync::Arc::new(fs_store::TursoStore::open_in_memory().await.unwrap());
    let backend = std::sync::Arc::new(LocalFolder::open(&deep).await.unwrap());
    let r = fs_files_core::FileRepo::new(store, common::ME, "main", backend);
    let a = r.create_folder(None, &"a".repeat(200)).await.unwrap();
    let b = r.create_folder(Some(a.id), &"b".repeat(200)).await.unwrap();
    let c = r.create_folder(None, &"c".repeat(200)).await.unwrap();
    // c under b would pass 4096 bytes together with the root, though the names alone are short
    let err = r.move_folder(c.id, Some(b.id)).await.unwrap_err();
    assert!(matches!(err, Error::InvalidName(_)), "{err}");
    assert_eq!(r.get_folder(c.id).await.unwrap().unwrap().parent, None);
}

#[tokio::test]
async fn keep_both_on_a_name_of_255_bytes_still_fits() {
    let e = env().await;
    let r = &e.repo;
    let long = format!("{}.txt", "n".repeat(251));
    let a = upload(r, None, &long, "text/plain", b"1").await;
    let b = upload(r, None, "x.txt", "text/plain", b"2").await;
    let moved = r
        .rename_file_as(b.id, &long, OnConflict::KeepBoth)
        .await
        .unwrap();
    assert!(
        moved.name.len() <= 255 && moved.name != a.name,
        "{}",
        moved.name.len()
    );
    assert!(moved.name.ends_with(" (2).txt"));
    assert_eq!(read_all(r, moved.id).await, b"2");
}

#[tokio::test]
async fn a_copy_is_not_reported_as_changed_and_is_not_an_upload() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"copy me").await;
    let c = r.copy_file(f.id, None).await.unwrap();
    let report = r.rescan(None).await.unwrap();
    assert!(
        report.updated.is_empty() && report.added.is_empty(),
        "{report:?}"
    );
    // the copy's record does not claim the source's upload: dropping that stale session cannot touch it
    assert_eq!(read_all(r, c.id).await, b"copy me");
}

#[tokio::test]
async fn a_file_where_a_folder_was_is_not_moved_by_a_folder_rename() {
    let e = env().await;
    let r = &e.repo;
    let a = r.create_folder(None, "A").await.unwrap();
    std::fs::remove_dir(root(&e).join("A")).unwrap();
    std::fs::write(root(&e).join("A"), b"a stranger's file").unwrap();
    let err = r.rename_folder(a.id, "B").await.unwrap_err();
    assert!(kind::<()>(Err(err)).ends_with("changed-outside"));
    assert_eq!(on_disk(&e, "A"), b"a stranger's file");
    assert!(!root(&e).join("B").exists());
    assert_eq!(r.get_folder(a.id).await.unwrap().unwrap().name, "A");
}

#[tokio::test]
async fn rescan_does_not_pull_a_stray_file_out_of_the_trash_folder_for_a_record() {
    let e = env().await;
    let r = &e.repo;
    let f = upload(r, None, "a.txt", "text/plain", b"mine!").await;
    std::fs::remove_file(root(&e).join("a.txt")).unwrap();
    std::fs::write(root(&e).join(format!(".fs-files/trash/{}", f.id)), b"stray").unwrap();
    let report = r.rescan(None).await.unwrap();
    assert_eq!(report.missing.len(), 1, "{report:?}");
    assert!(report.updated.is_empty());
    assert!(!root(&e).join("a.txt").exists());
    assert!(
        root(&e).join(format!(".fs-files/trash/{}", f.id)).is_file(),
        "the stray file stays where it was"
    );
}

#[cfg(feature = "s3")]
mod s3 {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::*;
    use common::fake_s3::Fake;
    use common::flaky::Flaky;
    use common::suite::{bytes, checksum};
    use fs_files_core::Transfer;
    use fs_files_core::{S3Backend, S3Options};

    fn backend(bucket: &Arc<Fake>) -> S3Backend {
        S3Backend::new(
            bucket.clone(),
            &S3Options {
                part_size: 5 << 20,
                ..S3Options::default()
            },
        )
        .unwrap()
    }

    #[tokio::test]
    async fn after_the_marker_no_more_bytes_are_taken() {
        let data = bytes(7_000_000);
        let mut n = 1;
        loop {
            let bucket = Arc::new(Fake::default());
            let b = backend(&bucket);
            for (i, c) in data.chunks(1_000_000).enumerate() {
                b.put("k", (i * 1_000_000) as u64, c).await.unwrap();
            }
            bucket.crash_at(n);
            let _ = b.commit("k").await;
            if !bucket.crashed() {
                break;
            }
            bucket.restart();
            let b2 = backend(&bucket);
            let held = b2.pending("k").await.unwrap();
            let more = b"more bytes";
            let wrote = b2.put("k", held, more).await.is_ok();
            let info = b2
                .commit("k")
                .await
                .unwrap_or_else(|e| panic!("cut at call {n}: {e}"));
            let whole = b2.get("k", 0, u32::MAX).await.unwrap();
            let mut extended = data.clone();
            extended.extend(more);
            if wrote {
                assert_eq!(info.checksum, checksum(&extended), "cut at {n}");
            } else {
                assert_eq!(
                    whole, data,
                    "cut at {n}: refused, the object is the original"
                );
            }
            n += 1;
        }
        assert!(n > 3);
    }

    #[tokio::test]
    async fn received_reports_the_committed_size_when_only_the_record_is_missing() {
        let store: Arc<dyn fs_store::Store> =
            Arc::new(fs_store::TursoStore::open_in_memory().await.unwrap());
        let flaky = Arc::new(Flaky {
            store,
            fail: Arc::default(),
            fail_delete: Arc::default(),
        });
        let backend = Arc::new(backend(&Arc::new(Fake::default())));
        let r = fs_files_core::FileRepo::with_backend(
            flaky.clone(),
            flaky.clone(),
            common::ME,
            "cloud",
            backend,
        );
        let (up, id) = r
            .start_upload(None, "a.bin", "application/octet-stream")
            .await
            .unwrap();
        up.write(0, b"sixteen bytes!!!").await.unwrap();
        flaky.fail.store(true, Ordering::SeqCst);
        assert!(up.finish().await.is_err());
        flaky.fail.store(false, Ordering::SeqCst);
        let again = r.resume_upload(&id).await.unwrap();
        assert_eq!(
            again.received().await.unwrap(),
            16,
            "all bytes are in place"
        );
        assert!(matches!(
            again.write(0, b"par").await,
            Err(Error::Conflict(_))
        ));
        let f = again.finish().await.unwrap();
        assert_eq!(read_all(&r, f.id).await, b"sixteen bytes!!!");
    }
}
