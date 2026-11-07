//! `LocalFolder`: the default backend, bytes as ordinary files under a root folder, **under their real names**.
//!
//! Pattern: Strategy (a `BlobBackend`, `Naming::ByName`) — safety first: keys are validated, no
//! symlink is followed, writes go to a pending file in the reserved folder (fsync after every
//! chunk), and `publish` moves it to its real path with an atomic no-replace rename. The
//! name-based operations (move, trash, scan) are in `named.rs`.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use tokio::io::AsyncWriteExt;

use super::io;
use super::path::{self, Reserved};

use crate::backend::digest::{Digests, hash_chunks};
use crate::backend::lock::KeyLocks;
use crate::backend::provider::{BlobBackend, BlobInfo};
use crate::backend::{Naming, Scanned, Spot};
use crate::error::{Error, Result};

/// Bytes as plain files under one root folder.
pub struct LocalFolder {
    pub(super) root: PathBuf,
    pub(super) locks: KeyLocks,
    pub(super) digests: Digests,
    /// Use the kernel's no-replace rename where there is one (always, except in tests of the fallback).
    pub(super) native: bool,
}

impl LocalFolder {
    /// Use `root`, creating it if needed. The root is resolved once (symlinks in the root path itself are the operator's choice).
    pub async fn open(root: impl AsRef<Path>) -> Result<Self> {
        tokio::fs::create_dir_all(root.as_ref()).await?;
        let root = tokio::fs::canonicalize(root.as_ref()).await?;
        for kind in [Reserved::Pending, Reserved::Trash] {
            tokio::fs::create_dir_all(path::reserved_dir(&root, kind)).await?;
        }
        Ok(Self {
            root,
            locks: KeyLocks::default(),
            digests: Digests::default(),
            native: true,
        })
    }

    /// Never use the kernel's no-replace rename, only the fallback (hard link + unlink for files). For
    /// tests: on Linux the fallback would otherwise never run.
    #[doc(hidden)]
    #[must_use]
    pub fn without_native_rename(mut self) -> Self {
        self.native = false;
        self
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
            .supported("blob.delete")
            .supported("blob.rename")
            .supported("blob.scan");
        c
    }

    fn local_root(&self) -> Option<&Path> {
        Some(&self.root)
    }

    fn kind(&self) -> &'static str {
        "local"
    }

    fn naming(&self) -> Naming {
        Naming::ByName
    }

    async fn get(&self, key: &str, offset: u64, length: u32) -> Result<Vec<u8>> {
        let p = path::final_path(&self.root, key)?;
        io::read_range(&p, offset, length)
            .await
            .map_err(|e| missing(e, key))
    }

    async fn stat(&self, key: &str) -> Result<Option<BlobInfo>> {
        let p = path::final_path(&self.root, key)?;
        Ok(io::stat_of(&p).await?.map(|(size, modified)| BlobInfo {
            size,
            checksum: String::new(),
            modified,
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
        let pending_dir = path::reserved_dir(&self.root, Reserved::Pending);
        io::make_all(&pending_dir).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .await?;
        file.write_all(data).await?;
        file.sync_data().await?;
        if held.is_none() {
            io::sync_dir(&pending_dir).await?;
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
        let (size, modified) = io::stat_of(&from)
            .await?
            .ok_or_else(|| Error::NotFound(format!("nothing pending for {key}").into()))?;
        let checksum = match self.digests.finish(key, size) {
            Some(c) => c,
            None => hash_chunks(size, |o, l| io::read_range(&from, o, l)).await?,
        };
        let dir = to.parent().ok_or_else(|| Error::InvalidKey(key.into()))?;
        io::make_all(dir).await?;
        path::final_path(&self.root, key)?; // re-check after creating directories
        // Never replaces: a blob that is already there is a name clash, and the upload stays pending.
        io::rename_new(&from, &to, self.native)
            .map_err(|e| io::named(e, &format!("commit {key}")))?;
        io::sync_dir(dir).await?;
        io::sync_dir(&path::reserved_dir(&self.root, Reserved::Pending)).await?;
        Ok(BlobInfo {
            size,
            checksum,
            modified,
        })
    }

    async fn publish(&self, pending: &str, key: &str) -> Result<BlobInfo> {
        self.do_publish(pending, key).await
    }

    async fn relocate(&self, from: &Spot, to: &Spot) -> Result<()> {
        self.do_relocate(from, to).await
    }

    async fn discard(&self, spot: &Spot) -> Result<()> {
        self.do_discard(spot).await
    }

    async fn inspect(&self, spot: &Spot) -> Result<Option<BlobInfo>> {
        self.do_inspect(spot).await
    }

    async fn has(&self, spot: &Spot) -> Result<bool> {
        self.do_has(spot)
    }

    async fn sweep_pending(&self, keep: &[String], older_than_secs: u64) -> Result<u32> {
        self.do_sweep_pending(keep, older_than_secs).await
    }

    async fn make_dir(&self, path: &str, parents: bool) -> Result<()> {
        self.do_make_dir(path, parents).await
    }

    async fn remove_dir(&self, path: &str) -> Result<()> {
        self.do_remove_dir(path).await
    }

    async fn scan(&self, dir: &str) -> Result<Vec<Scanned>> {
        self.do_scan(dir).await
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
            Error::NotFound(format!("blob {key}").into())
        }
        other => other,
    }
}
