//! A failure between the disk step and the database step loses no bytes.

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use common::disk::{on_disk, root};
use common::{ME, env, read_all, upload};
use fs_files_core::{BlobBackend, Entry, FileRepo, Files, LocalFolder, Spot, Transfer};
use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{Activity, LogEntry, ObjectId, Query, Store, StoredObject, TursoStore};

struct Flaky {
    store: Arc<dyn Store>,
    fail: Arc<AtomicBool>,
}

#[async_trait]
impl ObjectRepository for Flaky {
    async fn put(&self, o: StoredObject, actor: &str) -> fs_store::Result<Activity> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(fs_store::Error::Backend("simulated failure".into()));
        }
        Store::put(&*self.store, o, actor).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> fs_store::Result<Vec<Activity>> {
        Store::put_and_log(&*self.store, o, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> fs_store::Result<Option<StoredObject>> {
        Store::get(&*self.store, id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> fs_store::Result<Activity> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(fs_store::Error::Backend("simulated failure".into()));
        }
        Store::delete(&*self.store, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Flaky {
    async fn query(&self, q: &Query) -> fs_store::Result<Vec<StoredObject>> {
        Store::query(&*self.store, q).await
    }
    async fn count(&self, q: &Query) -> fs_store::Result<usize> {
        Store::count(&*self.store, q).await
    }
}

async fn flaky_repo(dir: &Path) -> (FileRepo, Arc<AtomicBool>, Arc<dyn Store>) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let fail = Arc::new(AtomicBool::new(false));
    let flaky = Arc::new(Flaky {
        store: store.clone(),
        fail: fail.clone(),
    });
    let backend = Arc::new(LocalFolder::open(dir.join("blobs")).await.unwrap());
    let repo = FileRepo::with_backend(flaky.clone(), flaky, ME, "main", backend);
    (repo, fail, store)
}

#[tokio::test]
async fn a_failed_database_step_undoes_the_disk_step() {
    let dir = tempfile::tempdir().unwrap();
    let blobs = dir.path().join("blobs");
    let (r, fail, _s) = flaky_repo(dir.path()).await;
    let d = r.create_folder(None, "d").await.unwrap();
    let (up, id) = r
        .start_upload(Some(d.id), "a.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"bytes").await.unwrap();
    let f = upload(&r, None, "keep.txt", "text/plain", b"keep").await;

    fail.store(true, Ordering::SeqCst);
    let err = up.finish().await.unwrap_err();
    assert!(
        !blobs.join("d/a.txt").exists(),
        "no user file without a record: {err}"
    );
    assert!(r.rename_file(f.id, "other.txt").await.is_err());
    assert!(blobs.join("keep.txt").is_file());
    assert!(r.rename_folder(d.id, "e").await.is_err());
    assert!(blobs.join("d").is_dir() && !blobs.join("e").exists());
    assert!(r.trash(f.id).await.is_err());
    assert!(blobs.join("keep.txt").is_file());
    assert!(r.create_folder(None, "new").await.is_err());
    assert!(!blobs.join("new").exists());
    fail.store(false, Ordering::SeqCst);

    // the upload was left intact and can be finished again
    assert_eq!(
        r.resume_upload(&id)
            .await
            .unwrap()
            .received()
            .await
            .unwrap(),
        5
    );
    let done = up.finish().await.unwrap();
    assert_eq!(read_all(&r, done.id).await, b"bytes");
    assert_eq!(std::fs::read(blobs.join("d/a.txt")).unwrap(), b"bytes");
}

#[tokio::test]
async fn after_a_crash_between_disk_and_database_rescan_loses_no_bytes() {
    let e = env().await;
    let r = &e.repo;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    // 1. crash after publish, before the record: bytes sit under their real name, nobody knows them
    b.put("up-1", 0, b"survivor").await.unwrap();
    b.publish("up-1", "crashed.txt").await.unwrap();
    // 2. crash after a rename on the disk, before the record
    let moved = upload(r, None, "before.txt", "text/plain", b"moved").await;
    std::fs::rename(root(&e).join("before.txt"), root(&e).join("after.txt")).unwrap();
    // 3. crash after "to trash", before the record: the bytes are in the trash, the record says live
    let trashed = upload(r, None, "t.txt", "text/plain", b"trash me").await;
    b.relocate(
        &Spot::Path("t.txt".into()),
        &Spot::Trash(trashed.id.to_string()),
    )
    .await
    .unwrap();

    assert!(
        r.open(moved.id).await.is_err(),
        "a clear error, not wrong data"
    );
    let report = r.rescan(None).await.unwrap();
    let added: Vec<_> = report
        .added
        .iter()
        .map(|x| match x {
            Entry::File(f) => f.name.clone(),
            Entry::Folder(f) => f.name.clone(),
        })
        .collect();
    assert_eq!(added.len(), 2, "{report:?}");
    assert!(added.contains(&"crashed.txt".to_string()) && added.contains(&"after.txt".to_string()));
    assert_eq!(
        report.missing.iter().map(|f| f.id).collect::<Vec<_>>(),
        [moved.id],
        "kept, reported"
    );
    assert_eq!(
        report.updated.iter().map(|f| f.id).collect::<Vec<_>>(),
        [trashed.id],
        "put back from the trash"
    );
    assert_eq!(on_disk(&e, "crashed.txt"), b"survivor");
    assert_eq!(on_disk(&e, "after.txt"), b"moved");
    assert_eq!(read_all(r, trashed.id).await, b"trash me");
    // the vanished record is still there
    assert!(r.get(moved.id).await.unwrap().is_some());
}
