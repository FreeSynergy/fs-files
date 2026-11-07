//! A store that fails on demand, to simulate a database that is down between two steps.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{Activity, LogEntry, ObjectId, Query, Store, StoredObject};

/// A store that can be told to fail writes (`fail`: put and delete) or only deletes (`fail_delete`).
pub struct Flaky {
    pub store: Arc<dyn Store>,
    pub fail: Arc<AtomicBool>,
    pub fail_delete: Arc<AtomicBool>,
}

#[async_trait]
impl ObjectRepository for Flaky {
    async fn put(&self, o: StoredObject, actor: &str) -> fs_objects::Result<Activity> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(fs_objects::Error::Backend("simulated failure".into()));
        }
        Store::put(&*self.store, o, actor).await
    }
    async fn put_and_log(
        &self,
        o: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> fs_objects::Result<Vec<Activity>> {
        Store::put_and_log(&*self.store, o, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> fs_objects::Result<Option<StoredObject>> {
        Store::get(&*self.store, id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> fs_objects::Result<Activity> {
        if self.fail.load(Ordering::SeqCst) || self.fail_delete.load(Ordering::SeqCst) {
            return Err(fs_objects::Error::Backend("simulated failure".into()));
        }
        Store::delete(&*self.store, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Flaky {
    async fn query(&self, q: &Query) -> fs_objects::Result<Vec<StoredObject>> {
        Store::query(&*self.store, q).await
    }
    async fn count(&self, q: &Query) -> fs_objects::Result<usize> {
        Store::count(&*self.store, q).await
    }
}
