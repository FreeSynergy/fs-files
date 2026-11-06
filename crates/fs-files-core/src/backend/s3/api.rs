//! [`ObjectApi`]: the few object-store calls the S3 backend needs.
//!
//! Pattern: Adapter (interface) — the backend talks to this narrow trait, not to a client library.
//! One adapter wraps the `object_store` crate; tests plug in a strict in-memory fake that enforces
//! S3's rules (minimum part size, part count), so the logic is tested without a server.

use async_trait::async_trait;

use crate::error::Result;

/// One object found by [`ObjectApi::list`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectEntry {
    /// Full key.
    pub key: String,
    /// Bytes.
    pub size: u64,
}

/// The calls of an S3-like store. All keys are full object keys; "not found" is `None` or `Ok`.
#[async_trait]
pub trait ObjectApi: Send + Sync {
    /// Write a whole object.
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<()>;
    /// Size of an object, `None` if missing.
    async fn head(&self, key: &str) -> Result<Option<u64>>;
    /// Bytes `start..end` (`end` <= size), `None` if the object is missing.
    async fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Option<Vec<u8>>>;
    /// Delete an object; a missing one is fine.
    async fn delete(&self, key: &str) -> Result<()>;
    /// Objects whose key starts with `prefix` (which ends in `/`), all pages.
    async fn list(&self, prefix: &str) -> Result<Vec<ObjectEntry>>;
    /// Start a multipart upload; returns its id.
    async fn create_multipart(&self, key: &str) -> Result<String>;
    /// Upload part number `index` (0-based); returns its etag.
    async fn put_part(&self, key: &str, id: &str, index: usize, data: Vec<u8>) -> Result<String>;
    /// Join the parts (etags in order) into the object. All parts but the last must be >= 5 MiB.
    async fn complete(&self, key: &str, id: &str, etags: Vec<String>) -> Result<()>;
    /// Throw a multipart upload away.
    async fn abort(&self, key: &str, id: &str) -> Result<()>;
}
