//! The [`Transfer`] contract: resumable upload and reading.
//!
//! Pattern: State (an upload is a session with a persisted position) + Repository (interface) —
//! the Rust form of `Transfer`, `Upload` and `Blob`. [`Blob`] is the capability other programs
//! read through: size and ranges, never a path.

use std::sync::Arc;

use async_trait::async_trait;
use fs_interfaces::Unsupported;
use fs_store::ObjectId;

use crate::error::Result;
use crate::model::{File, OnConflict};

/// Bytes behind a capability (`Blob` in `common.capnp`).
#[async_trait]
pub trait Blob: Send + Sync {
    /// Size in bytes and media type.
    async fn size(&self) -> Result<(u64, String)>;
    /// Up to `length` bytes from `offset`; fewer at the end, none past it.
    async fn read(&self, offset: u64, length: u32) -> Result<Vec<u8>>;
}

/// One resumable upload (`Upload` in `files.capnp`).
#[async_trait]
pub trait Upload: Send + Sync {
    /// Append `data`, which must start at `offset` = [`received`](Self::received); anything else
    /// is `Error::Offset` and nothing is written.
    async fn write(&self, offset: u64, data: &[u8]) -> Result<()>;
    /// Bytes held so far. Survives a restart: ask, then go on from there.
    async fn received(&self) -> Result<u64>;
    /// Verify the size, compute the checksum, and only then create the file.
    async fn finish(&self) -> Result<File>;
    /// Throw the upload and its bytes away.
    async fn abort(&self) -> Result<()>;
}

/// Getting bytes in and out.
#[async_trait]
pub trait Transfer: Send + Sync {
    /// `file.download`: the bytes of a live file.
    async fn open(&self, id: ObjectId) -> Result<Arc<dyn Blob>>;

    /// `file.upload`: start an upload into `folder`; returns the session and its id (keep the id to resume).
    /// An empty or generic `media_type` is detected from the first bytes at `finish`.
    async fn start_upload(
        &self,
        _folder: Option<ObjectId>,
        _name: &str,
        _media_type: &str,
    ) -> Result<(Arc<dyn Upload>, String)> {
        Err(Unsupported::new("file.upload", "this provider cannot take uploads").into())
    }
    /// `file.upload` with a rule for a taken name: `Refuse` fails at the start, `KeepBoth` starts an upload
    /// that will get a free name (`name (2).ext`). The default refuses.
    async fn start_upload_as(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
        on: OnConflict,
    ) -> Result<(Arc<dyn Upload>, String)> {
        match on {
            OnConflict::Refuse => self.start_upload(folder, name, media_type).await,
            OnConflict::KeepBoth => Err(Unsupported::new(
                "file.conflictRule",
                "this provider only knows the refuse rule",
            )
            .into()),
        }
    }
    /// `file.upload`: take up a started upload again, also after a restart.
    async fn resume_upload(&self, _id: &str) -> Result<Arc<dyn Upload>> {
        Err(Unsupported::new("file.upload", "this provider cannot take uploads").into())
    }
}
