//! The provider interface [`BlobBackend`]: where bytes are.
//!
//! Pattern: Strategy (interface) + Null Object — the Rust form of `BlobBackend` in `files.capnp`.
//! Like a parent class (ADR 0045): every backend has every method; what it does not override
//! answers an explicit [`Unsupported`], and `capabilities` reports the same. Reading is the basic
//! part and must be implemented.

use async_trait::async_trait;
use fs_interfaces::{Capabilities, Unsupported};

use crate::error::Result;

/// Size and checksum of a stored blob (`BlobInfo` in the contract).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobInfo {
    /// Bytes.
    pub size: u64,
    /// `blake3:<hex>` as returned by `commit`; empty from `stat`, which only knows the size
    /// (hashing a whole blob to answer a size question would be wrong; see `checksum_of`).
    pub checksum: String,
}

/// A place that holds bytes under keys: a local folder, an S3 bucket, later `WebDAV`.
///
/// A blob is written in order: any number of [`put`](Self::put) calls, each at the offset where
/// the previous one ended, then [`commit`](Self::commit) makes it readable. Until then it is
/// *pending*: invisible to `get` and `stat`, and its length is [`pending`](Self::pending).
///
/// Interface status: draft (ADR 0045).
#[async_trait]
pub trait BlobBackend: Send + Sync {
    /// What this backend can do, per capability id.
    fn capabilities(&self) -> Capabilities;
    /// The kind written in `BackendInfo` (`local`, `s3`, ...).
    fn kind(&self) -> &'static str;
    /// `blob.read`: up to `length` bytes from `offset` of a committed blob; fewer at the end,
    /// none past it. A missing blob is [`Error::NotFound`](crate::Error::NotFound).
    async fn get(&self, key: &str, offset: u64, length: u32) -> Result<Vec<u8>>;
    /// `blob.read`: the size of a committed blob, if it exists.
    async fn stat(&self, key: &str) -> Result<Option<BlobInfo>>;

    /// `blob.write`: append `data` to the pending blob. `offset` must equal what is already held
    /// (`Error::Offset` otherwise, nothing written). When this returns the bytes are durable.
    async fn put(&self, _key: &str, _offset: u64, _data: &[u8]) -> Result<()> {
        Err(Unsupported::new("blob.write", "this backend cannot be written").into())
    }
    /// `blob.write`: make the pending blob readable (atomically) and return size and checksum.
    async fn commit(&self, _key: &str) -> Result<BlobInfo> {
        Err(Unsupported::new("blob.write", "this backend cannot be written").into())
    }
    /// `blob.write`: bytes held by the pending blob of `key` (0 if none). Needed to resume an
    /// upload after a restart. Not in `files.capnp` yet (reported to the owner).
    async fn pending(&self, _key: &str) -> Result<u64> {
        Err(Unsupported::new("blob.write", "this backend cannot be written").into())
    }
    /// `blob.delete`: remove the committed and the pending blob of `key`. Missing is not an error.
    async fn delete(&self, _key: &str) -> Result<()> {
        Err(Unsupported::new("blob.delete", "this backend cannot delete").into())
    }
}
