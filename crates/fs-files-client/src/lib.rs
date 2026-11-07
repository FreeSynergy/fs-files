//! Stub for the red tests; replaced in the next commit.
//!
//! Pattern: none (stub).

use std::io;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use fs_files_core::{Blob, Cursor, File, Files, Folder, Page, Result, Transfer};
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

pub use fs_interfaces::rpc::client::Timeouts;

/// Stub.
pub struct RpcFiles;

impl RpcFiles {
    /// Stub.
    ///
    /// # Errors
    /// Never.
    pub fn connect_with(socket: &Path, _t: Timeouts) -> io::Result<Self> {
        Self::connect(socket)
    }

    /// Stub that connects to nothing and answers `Unsupported` everywhere.
    ///
    /// # Errors
    /// Never.
    pub fn connect(_socket: &Path) -> io::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl Files for RpcFiles {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("stub")
    }
    async fn get(&self, _id: ObjectId) -> Result<Option<File>> {
        Err(fs_interfaces::Unsupported::new("file.resolve", "stub").into())
    }
    async fn get_folder(&self, _id: ObjectId) -> Result<Option<Folder>> {
        Err(fs_interfaces::Unsupported::new("folder.resolve", "stub").into())
    }
    async fn list(&self, _folder: Option<ObjectId>, _cursor: Option<Cursor>) -> Result<Page> {
        Err(fs_interfaces::Unsupported::new("folder.list", "stub").into())
    }
}

#[async_trait]
impl Transfer for RpcFiles {
    async fn open(&self, _id: ObjectId) -> Result<Arc<dyn Blob>> {
        Err(fs_interfaces::Unsupported::new("file.download", "stub").into())
    }
}
