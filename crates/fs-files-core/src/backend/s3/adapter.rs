//! [`ObjectApi`] over the `object_store` crate.
//!
//! Pattern: Adapter — `ObjectStoreApi<T>` wraps any `ObjectStore + MultipartStore` (S3, but also the
//! crate's in-memory store, which the tests use) and maps its errors to ours. Error texts come from
//! the library and never contain the credentials (they are only used to sign requests).

use async_trait::async_trait;
use bytes::Bytes;
use object_store::multipart::{MultipartStore, PartId};
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};

use super::api::{ObjectApi, ObjectEntry};
use crate::error::{Error, Result};

/// An [`ObjectApi`] backed by an `object_store` implementation.
pub struct ObjectStoreApi<T>(pub T);

fn fail(e: &object_store::Error) -> Error {
    Error::Backend(e.to_string().into())
}

fn path(key: &str) -> Path {
    Path::from(key)
}

fn payload(data: Vec<u8>) -> PutPayload {
    PutPayload::from(Bytes::from(data))
}

#[async_trait]
impl<T: ObjectStore + MultipartStore> ObjectApi for ObjectStoreApi<T> {
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<()> {
        ObjectStoreExt::put(&self.0, &path(key), payload(data))
            .await
            .map(|_| ())
            .map_err(|e| fail(&e))
    }

    async fn head(&self, key: &str) -> Result<Option<u64>> {
        match self.0.head(&path(key)).await {
            Ok(meta) => Ok(Some(meta.size)),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(fail(&e)),
        }
    }

    async fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Option<Vec<u8>>> {
        match self.0.get_range(&path(key), start..end).await {
            Ok(bytes) => Ok(Some(bytes.to_vec())),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(fail(&e)),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        match self.0.delete(&path(key)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(fail(&e)),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectEntry>> {
        let dir = path(prefix.trim_end_matches('/'));
        let found = self
            .0
            .list_with_delimiter(Some(&dir))
            .await
            .map_err(|e| fail(&e))?;
        Ok(found
            .objects
            .into_iter()
            .map(|m| ObjectEntry {
                key: m.location.to_string(),
                size: m.size,
            })
            .collect())
    }

    async fn create_multipart(&self, key: &str) -> Result<String> {
        self.0
            .create_multipart(&path(key))
            .await
            .map_err(|e| fail(&e))
    }

    async fn put_part(&self, key: &str, id: &str, index: usize, data: Vec<u8>) -> Result<String> {
        let part = self
            .0
            .put_part(&path(key), &id.to_string(), index, payload(data))
            .await
            .map_err(|e| fail(&e))?;
        Ok(part.content_id)
    }

    async fn complete(&self, key: &str, id: &str, etags: Vec<String>) -> Result<()> {
        let parts = etags
            .into_iter()
            .map(|content_id| PartId { content_id })
            .collect();
        self.0
            .complete_multipart(&path(key), &id.to_string(), parts)
            .await
            .map(|_| ())
            .map_err(|e| fail(&e))
    }

    async fn abort(&self, key: &str, id: &str) -> Result<()> {
        match self.0.abort_multipart(&path(key), &id.to_string()).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(fail(&e)),
        }
    }
}
