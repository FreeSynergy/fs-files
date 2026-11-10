//! Durable file operations of the local folder.
//!
//! Pattern: none (small helpers) — fsync of files and directories, so a rename or an append that
//! returned is still there after a power loss.

use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::error::{Error, Result};
use crate::note::Note;

/// Flush a directory entry to disk (where the system has such a thing).
pub(super) async fn sync_dir(dir: &Path) -> Result<()> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || crate::platform::current().durability.sync_dir(&dir))
        .await
        .map_err(|e| Error::Backend(e.to_string().into()))??;
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

/// Size and modification time (ns since the epoch, 0 if unknown) of a file, `None` if it does not exist.
pub(super) async fn stat_of(path: &Path) -> Result<Option<(u64, i64)>> {
    match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() => Ok(Some((m.len(), modified_ns(&m)))),
        Ok(_) => Err(Error::Backend("not a regular file".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Modification time in nanoseconds since the epoch, 0 if unknown.
pub(super) fn modified_ns(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
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
/// On Linux this is one atomic `renameat2(RENAME_NOREPLACE)` (the platform part `Moves`). Without the flag (old kernel, a file system
/// that lacks it, other systems) a file is moved by hard link + unlink, which is atomic and never replaces;
/// a folder by "look, then rename", which is **not** atomic (see the comment in the body).
pub(super) fn rename_new(from: &Path, to: &Path, native: bool) -> std::io::Result<()> {
    if crate::platform::current()
        .moves
        .rename_new_native(from, to, native)?
    {
        return Ok(());
    }
    // A change of case only (`Other` to `OTHER`) on a disk that keeps one name for both is the same entry under a new spelling:
    // it is renamed in place, and nothing is replaced because nothing else has that name.
    if same_entry_new_spelling(from, to) {
        return std::fs::rename(from, to);
    }
    // Fallback. A file is moved by hard link + unlink, which refuses an existing target atomically
    // (POSIX `link`). A folder cannot be linked: "look, then rename" is the best there is, and a folder
    // created in between could be replaced if it is empty (never one with content).
    let from_meta = from.symlink_metadata()?;
    if from_meta.is_dir() {
        if to.symlink_metadata().is_ok() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        return std::fs::rename(from, to);
    }
    std::fs::hard_link(from, to)?;
    std::fs::remove_file(from)
}

/// `from` and `to` sit in one folder and their names differ only in what the disk ignores (the case, on Windows).
fn same_entry_new_spelling(from: &Path, to: &Path) -> bool {
    let (Some(a), Some(b)) = (
        from.file_name().and_then(|n| n.to_str()),
        to.file_name().and_then(|n| n.to_str()),
    ) else {
        return false;
    };
    let Some(dir) = from.parent().filter(|d| Some(*d) == to.parent()) else {
        return false;
    };
    // A folder can be case sensitive on Windows: there `Other` and `OTHER` are two entries and the rename would replace one.
    // The listing shows each entry in its own spelling, so an entry with exactly the new spelling is another one; on a folder
    // that ignores case the listing has only the old spelling.
    a != b
        && crate::platform::current().names.same_name(a, b)
        && !std::fs::read_dir(dir).is_ok_and(|mut entries| {
            entries.any(|e| e.is_ok_and(|e| e.file_name().to_str() == Some(b)))
        })
}

/// Turn an IO error of a name-based operation into the error the caller should see.
pub(super) fn named(e: std::io::Error, what: &str) -> Error {
    use std::io::ErrorKind as K;
    match e.kind() {
        K::AlreadyExists => Error::name_taken(format!(
            "{what}: the name is taken on the disk (by something fs-files does not know, or the file system \
             treats the names as the same); run rescan to take it over"
        )),
        K::DirectoryNotEmpty | K::NotADirectory | K::IsADirectory => Error::Conflict(
            format!(
                "{what}: a file is in the way or the folder holds things fs-files does not know (run rescan): {e}"
            )
            .into(),
        ),
        K::NotFound => Error::NotFound(what.into()),
        K::InvalidFilename => {
            Error::InvalidName(
                Note::from(format!("{what}: the file system refuses this name")).because("refused-by-disk"),
            )
        }
        _ if e.raw_os_error() == Some(36) => Error::InvalidName(
            Note::from(format!("{what}: name or path too long for this file system"))
                .because("too-long"),
        ),
        _ => Error::Io(e),
    }
}

/// Create `dir` and its parents.
pub(super) async fn make_all(dir: &Path) -> Result<()> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| named(e, "create folder"))
}
