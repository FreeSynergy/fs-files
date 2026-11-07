//! The name-based operations of the local folder: publish, relocate, discard, folders, scan.
//!
//! Pattern: none (small steps, no command objects) — each is: validate the spot, create missing parent folders,
//! do **one** atomic no-replace rename (or one create/remove), then fsync the directories touched.
//! Nothing here ever replaces a file: a taken target is `Error::Conflict`.

use std::path::Path;

use super::folder::LocalFolder;
use super::io;
use super::path::{self, Reserved};
use crate::backend::digest::hash_chunks;
use crate::backend::{BlobInfo, ScanKind, Scanned, Spot};
use crate::error::{Error, Result};

impl LocalFolder {
    /// Create the parents of `to`, then check again that no symlink is on the way.
    async fn prepare_target(&self, to: &Path) -> Result<()> {
        if let Some(parent) = to.parent() {
            io::make_all(parent).await?;
        }
        path::refuse_symlinks(&self.root, to)
    }

    /// fsync the folder that holds `p`.
    async fn sync_parent(&self, p: &Path) -> Result<()> {
        match p.parent() {
            Some(dir) => io::sync_dir(dir).await,
            None => Ok(()),
        }
    }

    pub(super) async fn do_publish(&self, pending: &str, key: &str) -> Result<BlobInfo> {
        let from = path::pending_path(&self.root, pending)?;
        let to = path::final_path(&self.root, key)?;
        let _guard = self.locks.lock(pending).await;
        let (size, modified) = io::stat_of(&from)
            .await?
            .ok_or_else(|| Error::NotFound(format!("nothing pending for {pending}")))?;
        let checksum = match self.digests.finish(pending, size) {
            Some(c) => c,
            None => hash_chunks(size, |o, l| io::read_range(&from, o, l)).await?,
        };
        self.prepare_target(&to).await?;
        io::rename_new(&from, &to, self.native)
            .map_err(|e| io::named(e, &format!("publish {key}")))?;
        self.sync_parent(&to).await?;
        self.sync_parent(&from).await?;
        Ok(BlobInfo {
            size,
            checksum,
            modified,
        })
    }

    pub(super) async fn do_relocate(&self, from: &Spot, to: &Spot) -> Result<()> {
        let source = path::spot_path(&self.root, from)?;
        let target = path::spot_path(&self.root, to)?;
        self.prepare_target(&target).await?;
        io::rename_new(&source, &target, self.native)
            .map_err(|e| io::named(e, &describe(from, to)))?;
        self.sync_parent(&target).await?;
        self.sync_parent(&source).await
    }

    pub(super) async fn do_discard(&self, spot: &Spot) -> Result<()> {
        let p = path::spot_path(&self.root, spot)?;
        if let Spot::Pending(key) = spot {
            let _guard = self.locks.lock(key).await;
            self.digests.forget(key);
            return io::remove_quiet(&p).await;
        }
        io::remove_quiet(&p).await
    }

    pub(super) async fn do_make_dir(&self, key: &str, parents: bool) -> Result<()> {
        let p = path::final_path(&self.root, key)?;
        if parents {
            self.prepare_target(&p).await?;
            return match tokio::fs::create_dir(&p).await {
                Ok(()) => self.sync_parent(&p).await,
                Err(e) if p.is_dir() && e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(io::named(e, &format!("create folder {key}"))),
            };
        }
        tokio::fs::create_dir(&p)
            .await
            .map_err(|e| io::named(e, &format!("create folder {key}")))?;
        self.sync_parent(&p).await
    }

    pub(super) async fn do_remove_dir(&self, key: &str) -> Result<()> {
        let p = path::final_path(&self.root, key)?;
        match tokio::fs::remove_dir(&p).await {
            Ok(()) => self.sync_parent(&p).await,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io::named(e, &format!("remove folder {key}"))),
        }
    }

    pub(super) async fn do_inspect(&self, spot: &Spot) -> Result<Option<BlobInfo>> {
        let p = path::spot_path(&self.root, spot)?;
        let Some((size, modified)) = io::stat_of(&p).await? else {
            return Ok(None);
        };
        let checksum = hash_chunks(size, |o, l| io::read_range(&p, o, l)).await?;
        Ok(Some(BlobInfo {
            size,
            checksum,
            modified,
        }))
    }

    pub(super) fn do_has(&self, spot: &Spot) -> Result<bool> {
        Ok(path::spot_path(&self.root, spot)?
            .symlink_metadata()
            .is_ok())
    }

    pub(super) async fn do_sweep_pending(&self, keep: &[String], older: u64) -> Result<u32> {
        let wanted: std::collections::HashSet<String> = keep
            .iter()
            .map(|k| format!("{}.part", blake3::hash(k.as_bytes()).to_hex()))
            .collect();
        let dir = path::reserved_dir(&self.root, Reserved::Pending);
        path::refuse_symlinks(&self.root, &dir)?;
        let mut reader = tokio::fs::read_dir(&dir).await?;
        let (mut removed, limit) = (0, std::time::Duration::from_secs(older));
        while let Some(entry) = reader.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().await?;
            let old = meta
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age >= limit);
            if meta.is_file()
                && std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("part"))
                && !wanted.contains(&name)
                && old
            {
                io::remove_quiet(&entry.path()).await?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    pub(super) async fn do_scan(&self, dir: &str) -> Result<Vec<Scanned>> {
        let p = if dir.is_empty() {
            self.root.clone()
        } else {
            path::final_path(&self.root, dir)?
        };
        let mut reader = tokio::fs::read_dir(&p)
            .await
            .map_err(|e| io::named(e, &format!("scan {dir}")))?;
        let mut out = Vec::new();
        while let Some(entry) = reader.next_entry().await? {
            let raw = entry.file_name();
            let Some(name) = raw.to_str().map(str::to_string) else {
                out.push(skipped(
                    raw.to_string_lossy().into_owned(),
                    "name is not valid UTF-8",
                ));
                continue;
            };
            if dir.is_empty() && name == crate::backend::RESERVED_NAME {
                continue; // the program's own folder is never a user entry
            }
            if !path::name_ok(&name) {
                out.push(skipped(
                    name,
                    "name not accepted (control character or separator)",
                ));
                continue;
            }
            let meta = entry.metadata().await?; // does not follow a symlink
            let kind = if meta.file_type().is_symlink() {
                ScanKind::Skipped("symlink".into())
            } else if meta.is_dir() {
                ScanKind::Dir
            } else if meta.is_file() {
                ScanKind::File
            } else {
                ScanKind::Skipped("not a regular file".into())
            };
            let modified = io::modified_ns(&meta);
            let size = if meta.is_file() { meta.len() } else { 0 };
            out.push(Scanned {
                name,
                kind,
                size,
                modified,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Where the pending uploads are (for the tests of the crash behaviour).
    pub fn pending_dir(&self) -> std::path::PathBuf {
        path::reserved_dir(&self.root, Reserved::Pending)
    }
}

fn skipped(name: String, why: &str) -> Scanned {
    Scanned {
        name,
        kind: ScanKind::Skipped(why.into()),
        size: 0,
        modified: 0,
    }
}

fn describe(from: &Spot, to: &Spot) -> String {
    let show = |s: &Spot| match s {
        Spot::Path(k) => k.clone(),
        Spot::Pending(_) => "an unfinished upload".into(),
        Spot::Trash(_) => "the trash".into(),
    };
    format!("move {} to {}", show(from), show(to))
}
