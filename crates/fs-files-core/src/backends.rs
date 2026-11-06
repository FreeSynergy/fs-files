//! The [`Backends`] contract: which providers hold the bytes.
//!
//! Pattern: Factory + Registry (interface) — `add` builds a backend from a configuration, checks
//! it, and swaps it in atomically (ADR 0045: configuration, checked first, then swapped).

use async_trait::async_trait;
use fs_interfaces::Unsupported;

use crate::error::Result;
use crate::model::BackendInfo;

/// Backends, swappable at runtime.
#[async_trait]
pub trait Backends: Send + Sync {
    /// `backend.list`.
    async fn list_backends(&self) -> Result<Vec<BackendInfo>>;

    /// `backend.add`: `config` is JSON (`{"id": .., "kind": "local" | "s3", ..}`). It is checked
    /// (built, then written, read and deleted once) before it is swapped in; on failure nothing changes.
    async fn add_backend(&self, _config: &str) -> Result<BackendInfo> {
        Err(Unsupported::new("backend.add", "this provider has a fixed backend").into())
    }
    /// `backend.remove`: refused while files or uploads are on it, and for the active backend.
    async fn remove_backend(&self, _id: &str) -> Result<()> {
        Err(Unsupported::new("backend.remove", "this provider has a fixed backend").into())
    }
}
