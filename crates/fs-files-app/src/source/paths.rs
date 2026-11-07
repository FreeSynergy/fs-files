//! Paths the person typed.
//!
//! Pattern: Specification — a path of the person's own computer. The core is never given a path: only
//! the name of the file, which it checks itself. A symbolic link at the end of a path is not followed,
//! neither to read nor to write, and what was opened must be what was looked at. Links further up the
//! path are the person's own folders (a home can be one) and are followed by the system.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use fs_components_workbenches::file_manager::{FileError, FileProblem};

use crate::words::say;

fn refused(why: &str) -> FileError {
    FileError::failed(why)
}

/// A plain file to read: opened, with its size and its name.
///
/// # Errors
/// [`FileError`] if the path is empty, a link, not a plain file, or was swapped while opening.
pub(super) fn open_source(path: &str) -> Result<(File, u64, String), FileError> {
    if path.trim().is_empty() {
        return Err(refused(say::NO_PATH));
    }
    let path = Path::new(path);
    let name = path
        .file_name()
        .ok_or_else(|| FileError::new(FileProblem::InvalidName, say::NO_FILE_NAME))?
        .to_str()
        .ok_or_else(|| FileError::new(FileProblem::InvalidName, say::NOT_UTF8))?
        .to_string();
    let seen = fs::symlink_metadata(path).map_err(|e| FileError::failed(e.to_string()))?;
    if seen.file_type().is_symlink() {
        return Err(refused(say::SYMLINK));
    }
    if !seen.is_file() {
        return Err(refused(say::NOT_A_FILE));
    }
    let file = File::open(path).map_err(|e| FileError::failed(e.to_string()))?;
    let opened = file
        .metadata()
        .map_err(|e| FileError::failed(e.to_string()))?;
    if !same_file(&seen, &opened) {
        return Err(refused(say::SWAPPED));
    }
    Ok((file, opened.len(), name))
}

#[cfg(unix)]
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

#[cfg(not(unix))]
fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.len() == b.len() && b.is_file()
}

/// Where to write a file of this name: `path` must not exist; a folder means inside it.
///
/// # Errors
/// [`FileError`] if the path is empty, a link, taken, or its folder does not exist.
pub(super) fn target_for(path: &str, name: &str) -> Result<PathBuf, FileError> {
    if path.trim().is_empty() {
        return Err(refused(say::NO_PATH));
    }
    let mut target = PathBuf::from(path);
    if let Ok(seen) = fs::symlink_metadata(&target) {
        if seen.file_type().is_symlink() {
            return Err(refused(say::SYMLINK));
        }
        if !seen.is_dir() {
            return Err(refused(say::TAKEN_PATH));
        }
        target.push(name);
        if fs::symlink_metadata(&target).is_ok() {
            return Err(refused(say::TAKEN_PATH));
        }
        return Ok(target);
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if !parent.is_dir() {
        return Err(refused(say::NO_PARENT));
    }
    Ok(target)
}
