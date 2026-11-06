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

/// Move `from` to `to` (file or folder) and **never replace** what is at `to`.
///
/// On Linux this is one atomic `renameat2(RENAME_NOREPLACE)`. Where the kernel or the file system
/// lacks the flag, or on other systems, it falls back to "look, then rename": that is **not**
/// atomic, and a rename onto an empty folder created in between would replace that empty folder
/// (never a file, never a folder with content).
pub(super) fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        use rustix::io::Errno;
        match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
            Ok(()) => return Ok(()),
            Err(e) if e == Errno::INVAL || e == Errno::NOSYS => {}
            Err(e) => return Err(e.into()),
        }
    }
    if to.symlink_metadata().is_ok() {
        return Err(std::io::ErrorKind::AlreadyExists.into());
    }
    std::fs::rename(from, to)
}

/// Turn an IO error of a name-based operation into the error the caller should see.
pub(super) fn named(e: std::io::Error, what: &str) -> Error {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::AlreadyExists => Error::name_taken(format!(
            "{what}: the name is taken on the disk (by something fs-files does not know, or the file system \
             treats the names as the same); run rescan to take it over"
        )),
        K::DirectoryNotEmpty | K::NotADirectory | K::IsADirectory => Error::Conflict(format!(
            "{what}: a file is in the way or the folder holds things fs-files does not know (run rescan): {e}"
        )),
        K::NotFound => Error::NotFound(what.to_string()),
        K::InvalidFilename => {
            Error::InvalidName(format!("{what}: the file system refuses this name"))
        }
        _ if e.raw_os_error() == Some(36) => Error::InvalidName(format!(
            "{what}: name or path too long for this file system"
        )),
        _ => Error::Io(e),
    }
}

/// Create `dir` and its parents.
pub(super) async fn make_all(dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| named(e, "create folder"))
}
