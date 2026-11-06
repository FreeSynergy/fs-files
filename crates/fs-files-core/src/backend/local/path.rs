//! Turning a validated key into a path under the root, and refusing symlinks.
//!
//! Pattern: Specification — the path rules of the local folder in one place. A key is checked by
//! `check_key` (no `..`, no absolute path, no NUL, no hidden segment); on top, no component of
//! the resulting path may be a symlink, so a link planted under the root cannot lead out of it.

use std::path::{Path, PathBuf};

use crate::backend::key::check_key;
use crate::error::{Error, Result};

/// Directory (below the root) that holds pending blobs. No key can name it: segments never start with `.`.
pub(super) const PENDING_DIR: &str = ".pending";

/// Path of the committed blob `key`.
pub(super) fn final_path(root: &Path, key: &str) -> Result<PathBuf> {
    let segments = check_key(key)?;
    let mut path = root.to_path_buf();
    path.extend(&segments);
    refuse_symlinks(root, &path)?;
    Ok(path)
}

/// Path of the pending blob `key`: flat, `/` replaced by `~` (not allowed in keys, so unambiguous).
pub(super) fn pending_path(root: &Path, key: &str) -> Result<PathBuf> {
    check_key(key)?;
    let path = root
        .join(PENDING_DIR)
        .join(format!("{}.part", key.replace('/', "~")));
    refuse_symlinks(root, &path)?;
    Ok(path)
}

/// Fail if any existing component between `root` (exclusive) and `path` is a symlink.
fn refuse_symlinks(root: &Path, path: &Path) -> Result<()> {
    let rest = path
        .strip_prefix(root)
        .map_err(|_| Error::InvalidKey("path leaves the root".into()))?;
    let mut current = root.to_path_buf();
    for part in rest.components() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::InvalidKey("a symlink is in the way".into()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
