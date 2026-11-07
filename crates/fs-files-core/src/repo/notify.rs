//! Telling subscribers what the repository wrote.
//!
//! Pattern: Decorator — [`Notifying`] wraps the `ObjectRepository` of the repository and implements
//! the same trait; every write of a file or folder is told to the [`Registry`] after the store has
//! accepted it. One place sees every write (upload finish, folder ops, trash, rescan), so no
//! operation can forget to notify. Upload sessions are bookkeeping and are never told.

use std::sync::Arc;

use async_trait::async_trait;
use fs_objects::traits::ObjectRepository;
use fs_objects::{Activity, ActivityKind, LogEntry, ObjectId, Result, StoredObject};

use super::consts::UPLOAD_TYPE;
use crate::feed::{FileChange, Registry};

/// An [`ObjectRepository`] that tells a [`Registry`] about each write of a file or folder.
pub(super) struct Notifying {
    pub(super) inner: Arc<dyn ObjectRepository>,
    pub(super) feed: Arc<Registry>,
}

impl Notifying {
    fn told(&self, activity: &Activity, object_type: &str) {
        let id = activity.object;
        if object_type == UPLOAD_TYPE {
            return;
        }
        match activity.kind {
            ActivityKind::Create => self.feed.notify(&FileChange::Created(id)),
            ActivityKind::Delete => self.feed.notify(&FileChange::Deleted(id)),
            _ => self.feed.notify(&FileChange::Updated(id)),
        }
    }
}

#[async_trait]
impl ObjectRepository for Notifying {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        let object_type = object.envelope.object_type.clone();
        let activity = self.inner.put(object, actor).await?;
        self.told(&activity, &object_type);
        Ok(activity)
    }

    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        let object_type = object.envelope.object_type.clone();
        let id = object.envelope.id;
        let activities = self.inner.put_and_log(object, actor, extra).await?;
        if let Some(a) = activities.iter().find(|a| a.object == id) {
            self.told(a, &object_type);
        }
        Ok(activities)
    }

    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.inner.get(id).await
    }

    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        let object_type = self
            .inner
            .get(id)
            .await?
            .map(|o| o.envelope.object_type)
            .unwrap_or_default();
        let activity = self.inner.delete(id, actor).await?;
        self.told(&activity, &object_type);
        Ok(activity)
    }
}
