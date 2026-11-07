//! Adapter from the `Store` facade to the two narrow traits the files program needs.
//!
//! Pattern: Adapter — `FileRepo::new` takes `Arc<dyn Store>`, but the repository itself only sees
//! `dyn ObjectRepository` and `dyn ObjectQuery` (interface segregation, ADR 0043).

use std::sync::Arc;

use async_trait::async_trait;
use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{
    Activity, LogEntry, ObjectId, Query, Result, Store, StoreId, StoreInfo, StoredObject,
};

/// Presents an `Arc<dyn Store>` as the narrow read / write traits.
pub(super) struct StoreAdapter(pub(super) Arc<dyn Store>);

#[async_trait]
impl ObjectRepository for StoreAdapter {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        Store::put(&*self.0, object, actor).await
    }

    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        Store::put_and_log(&*self.0, object, actor, extra).await
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        Store::get(&*self.0, id).await
    }

    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        Store::delete(&*self.0, id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for StoreAdapter {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        Store::query(&*self.0, query).await
    }

    async fn count(&self, query: &Query) -> Result<usize> {
        Store::count(&*self.0, query).await
    }
}

impl StoreInfo for StoreAdapter {
    fn identity(&self) -> StoreId {
        Store::identity(&*self.0)
    }

    fn is_encrypted(&self) -> bool {
        Store::is_encrypted(&*self.0)
    }
}
