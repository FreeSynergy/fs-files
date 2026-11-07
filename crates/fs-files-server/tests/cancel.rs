//! A client that goes away cannot leave a write half done: the disk and the database stay in step.

mod common;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::{ME, caps, list_all, with_repo};
use fs_files_core::{FileRepo, LocalFolder};
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Activity, LogEntry, ObjectId, Query, Result, Store, StoredObject, TursoStore};
use tokio::sync::{Notify, watch};

/// A store whose writes wait at a gate while it is closed.
struct Gated {
    store: Arc<dyn Store>,
    reached: Notify,
    gate: watch::Sender<bool>,
}

impl Gated {
    async fn wait(&self) {
        if !*self.gate.borrow() {
            self.reached.notify_one();
            let mut open = self.gate.subscribe();
            open.wait_for(|open| *open).await.unwrap();
        }
    }
}

#[async_trait]
impl ObjectRepository for Gated {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.wait().await;
        self.store.put(object, actor).await
    }

    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.wait().await;
        self.store.put_and_log(object, actor, extra).await
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.store.get(id).await
    }

    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.wait().await;
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Gated {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        self.store.query(query).await
    }

    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

async fn gated() -> (Arc<Gated>, Arc<FileRepo>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let gate = Arc::new(Gated {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        reached: Notify::new(),
        gate: watch::channel(true).0,
    });
    let backend = Arc::new(LocalFolder::open(dir.path().join("files")).await.unwrap());
    let repo = FileRepo::with_backend(gate.clone(), gate.clone(), ME, "main", backend);
    (gate, Arc::new(repo), dir)
}

/// Wait until a write stands at the closed gate, then drop the call that made it.
async fn drop_at_the_gate<T>(gate: &Gated, call: impl Future<Output = T>) {
    let reached = gate.reached.notified();
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            () = reached => {}
            _ = call => panic!("the call answered before its write"),
        }
    })
    .await
    .expect("the call reached no write");
    // The call is gone now; give the server the time to hear of it, as the write stands at the gate.
    tokio::time::sleep(Duration::from_millis(200)).await;
}

async fn appears(c: &common::Caps, name: &str) -> bool {
    for _ in 0..100 {
        if list_all(c, None).await.iter().any(|e| e.0 == name) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test(flavor = "current_thread")]
async fn a_folder_made_by_a_call_that_was_dropped_at_the_database_step_is_still_recorded() {
    let (gate, repo, dir) = gated().await;
    with_repo(repo, |root| async move {
        let c = caps(&root).await;
        gate.gate.send_replace(false);
        let mut make = c.folders.create_folder_request();
        make.get().init_parent();
        make.get().set_name("Half");
        drop_at_the_gate(&gate, make.send().promise).await;
        assert!(list_all(&c, None).await.is_empty(), "the gate holds");
        assert!(
            dir.path().join("files/Half").is_dir(),
            "the disk step is done"
        );

        gate.gate.send_replace(true);
        assert!(appears(&c, "Half").await, "the write stopped halfway");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_upload_finished_by_a_call_that_was_dropped_still_becomes_a_file() {
    let (gate, repo, dir) = gated().await;
    with_repo(repo, |root| async move {
        let c = caps(&root).await;
        let (upload, _) = common::start(&c, None, "late.txt").await.unwrap();
        let mut write = upload.write_request();
        write.get().set_offset(0);
        write.get().set_data(b"all the bytes");
        write.send().promise.await.unwrap();

        gate.gate.send_replace(false);
        drop_at_the_gate(&gate, upload.finish_request().send().promise).await;
        gate.gate.send_replace(true);
        assert!(appears(&c, "late.txt").await, "the finish stopped halfway");
        assert_eq!(
            std::fs::read(dir.path().join("files/late.txt")).unwrap(),
            b"all the bytes"
        );
    })
    .await;
}
