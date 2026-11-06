//! `LocalFolder`: the default backend, bytes as ordinary files under a root folder.
//!
//! Pattern: Strategy (a `BlobBackend`) — stage 1 provider. Safety first: keys are validated, no
//! symlink is followed, writes go to a pending file (fsync after every chunk) and `commit`
//! renames it into place (atomic) and syncs the directory.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use tokio::io::AsyncWriteExt;

use super::{io, path};

use crate::backend::digest::{Digests, hash_chunks};
use crate::backend::lock::KeyLocks;
use crate::backend::provider::{BlobBackend, BlobInfo};
use crate::error::{Error, Result};

/// Bytes as plain files under one root folder.
pub struct LocalFolder {
    root: PathBuf,
    locks: KeyLocks,
    digests: Digests,
}

impl LocalFolder {
    /// Use `root`, creating it if needed. The root is resolved once (symlinks in the root path itself are the operator's choice).
    pub async fn open(root: impl AsRef<Path>) -> Result<Self> {
        tokio::fs::create_dir_all(root.as_ref()).await?;
        let root = tokio::fs::canonicalize(root.as_ref()).await?;
        tokio::fs::create_dir_all(root.join(path::PENDING_DIR)).await?;
        Ok(Self {
            root,
            locks: KeyLocks::default(),
            digests: Digests::default(),
        })
    }

    /// The resolved root folder.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[async_trait]
impl BlobBackend for LocalFolder {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("local-folder");
        c.supported("blob.read")
            .supported("blob.write")
            .supported("blob.delete");
        c
    }

    fn kind(&self) -> &'static str {
        "local"
    }

    async fn get(&self, key: &str, offset: u64, length: u32) -> Result<Vec<u8>> {
        let p = path::final_path(&self.root, key)?;
        io::read_range(&p, offset, length)
            .await
            .map_err(|e| missing(e, key))
    }

    async fn stat(&self, key: &str) -> Result<Option<BlobInfo>> {
        let p = path::final_path(&self.root, key)?;
        Ok(io::len_of(&p).await?.map(|size| BlobInfo {
            size,
            checksum: String::new(),
        }))
    }

    async fn put(&self, key: &str, offset: u64, data: &[u8]) -> Result<()> {
        let p = path::pending_path(&self.root, key)?;
        let _guard = self.locks.lock(key).await;
        let held = io::len_of(&p).await?;
        if held.unwrap_or(0) != offset {
            return Err(Error::Offset {
                expected: held.unwrap_or(0),
                got: offset,
            });
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .await?;
        file.write_all(data).await?;
        file.sync_data().await?;
        if held.is_none() {
            io::sync_dir(&self.root.join(path::PENDING_DIR)).await?;
        }
        self.digests.feed(key, offset, data);
        Ok(())
    }

    async fn pending(&self, key: &str) -> Result<u64> {
        let p = path::pending_path(&self.root, key)?;
        Ok(io::len_of(&p).await?.unwrap_or(0))
    }

    async fn commit(&self, key: &str) -> Result<BlobInfo> {
        let from = path::pending_path(&self.root, key)?;
        let to = path::final_path(&self.root, key)?;
        let _guard = self.locks.lock(key).await;
        let size = io::len_of(&from)
            .await?
            .ok_or_else(|| Error::NotFound(format!("nothing pending for {key}")))?;
        let checksum = match self.digests.finish(key, size) {
            Some(c) => c,
            None => hash_chunks(size, |o, l| io::read_range(&from, o, l)).await?,
        };
        let dir = to.parent().ok_or_else(|| Error::InvalidKey(key.into()))?;
        tokio::fs::create_dir_all(dir).await?;
        path::final_path(&self.root, key)?; // re-check after creating directories
        tokio::fs::rename(&from, &to).await?;
        io::sync_dir(dir).await?;
        io::sync_dir(&self.root.join(path::PENDING_DIR)).await?;
        Ok(BlobInfo { size, checksum })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let committed = path::final_path(&self.root, key)?;
        let pending = path::pending_path(&self.root, key)?;
        let _guard = self.locks.lock(key).await;
        self.digests.forget(key);
        io::remove_quiet(&committed).await?;
        io::remove_quiet(&pending).await
    }
}

fn missing(e: Error, key: &str) -> Error {
    match e {
        Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            Error::NotFound(format!("blob {key}"))
        }
        other => other,
    }
}
