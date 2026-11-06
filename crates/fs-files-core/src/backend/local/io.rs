//! Durable file operations of the local folder.
//!
//! Pattern: none (small helpers) — fsync of files and directories, so a rename or an append that
//! returned is still there after a power loss.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::error::{Error, Result};

/// Flush a directory entry to disk (Unix; elsewhere a no-op).
pub(super) async fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        tokio::fs::File::open(dir).await?.sync_all().await?;
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Length of a file, `None` if it does not exist.
pub(super) async fn len_of(path: &Path) -> Result<Option<u64>> {
    match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() => Ok(Some(m.len())),
        Ok(_) => Err(Error::Backend("not a regular file".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Read up to `length` bytes at `offset`.
pub(super) async fn read_range(path: &Path, offset: u64, length: u32) -> Result<Vec<u8>> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(offset)).await?;
    let mut out = Vec::new();
    file.take(u64::from(length)).read_to_end(&mut out).await?;
    Ok(out)
}

/// Remove a file; a missing file is fine.
pub(super) async fn remove_quiet(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}
