//! Turning a key into a path under the root, and refusing symlinks.
//!
//! Pattern: Specification — the path rules of the local folder in one place. A key is a path of
//! real names; every segment is checked (no `.`/`..`, no separator, no control character, not the
//! reserved folder), and no existing component of the resulting path may be a symlink, so a link
//! planted under the root cannot lead out of it. Pending uploads and the trash live in the one
//! reserved folder and are addressed by a hash or an id, never by a user-chosen name.

use std::path::{Path, PathBuf};

use crate::backend::Spot;
use crate::backend::key::RESERVED_NAME;
use crate::error::{Error, Result};
use crate::note::Note;

/// Longest segment (a file name) in bytes: what Linux file systems accept.
pub(super) const MAX_NAME: usize = 255;
/// Longest key in bytes (`PATH_MAX`).
pub(super) const MAX_PATH: usize = 4095;
const PENDING: &str = "pending";
const TRASH: &str = "trash";

/// The segments of a key made of real names, or why it is refused.
pub(super) fn check_path(key: &str) -> Result<Vec<&str>> {
    let bad = |why: &str| {
        Err(Error::InvalidKey(
            Note::from(format!("{why}: {key:?}")).because(why),
        ))
    };
    if key.is_empty() || key.len() > MAX_PATH {
        return bad("empty or too long");
    }
    let segments: Vec<&str> = key.split('/').collect();
    for s in &segments {
        if s.is_empty() || s.len() > MAX_NAME {
            return bad("empty segment or name longer than 255 bytes");
        }
        if *s == "." || *s == ".." {
            return bad("dot segment");
        }
        if s.chars().any(|c| c.is_control() || c == '\\') {
            return bad("separator or control character in a name");
        }
        if let Some(why) = crate::platform::current().names.refuse(s) {
            return bad(why);
        }
    }
    if segments[0].eq_ignore_ascii_case(RESERVED_NAME) {
        return bad("the name is reserved for the program");
    }
    Ok(segments)
}

/// True if `name` could be a segment of a key (used by `scan` to skip odd entries).
pub(super) fn name_ok(name: &str) -> bool {
    check_path(name).is_ok()
}

/// Path of the committed blob or folder `key`.
pub(super) fn final_path(root: &Path, key: &str) -> Result<PathBuf> {
    let segments = check_path(key)?;
    // The limit is on the whole path, root included: the file system would answer "name too long".
    if root.as_os_str().len() + 1 + key.len() > MAX_PATH {
        return Err(Error::InvalidKey(
            Note::from(format!(
                "path too long for this file system ({MAX_PATH} bytes with the root): {key:?}"
            ))
            .because("path-too-long"),
        ));
    }
    let mut path = root.to_path_buf();
    path.extend(&segments);
    refuse_symlinks(root, &path)?;
    Ok(path)
}

/// The reserved folder `kind` below the root.
pub(super) fn reserved_dir(root: &Path, kind: Reserved) -> PathBuf {
    root.join(RESERVED_NAME).join(match kind {
        Reserved::Pending => PENDING,
        Reserved::Trash => TRASH,
    })
}

/// Which part of the reserved folder.
#[derive(Clone, Copy)]
pub(super) enum Reserved {
    Pending,
    Trash,
}

/// Path of the pending blob `key`: flat, named by the hash of the key (a key can be long and has `/`).
pub(super) fn pending_path(root: &Path, key: &str) -> Result<PathBuf> {
    check_path(key)?;
    let name = format!("{}.part", blake3::hash(key.as_bytes()).to_hex());
    let path = reserved_dir(root, Reserved::Pending).join(name);
    refuse_symlinks(root, &path)?;
    Ok(path)
}

/// Path of the trash entry `id` (letters, digits, `-`, `_` only).
pub(super) fn trash_path(root: &Path, id: &str) -> Result<PathBuf> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !ok {
        return Err(Error::InvalidKey(
            format!("trash id not allowed: {id:?}").into(),
        ));
    }
    let path = reserved_dir(root, Reserved::Trash).join(id);
    refuse_symlinks(root, &path)?;
    Ok(path)
}

/// The path of any spot.
pub(super) fn spot_path(root: &Path, spot: &Spot) -> Result<PathBuf> {
    match spot {
        Spot::Path(k) => final_path(root, k),
        Spot::Pending(k) => pending_path(root, k),
        Spot::Trash(id) => trash_path(root, id),
    }
}

/// Fail if any existing component between `root` (exclusive) and `path` is a symlink.
pub(super) fn refuse_symlinks(root: &Path, path: &Path) -> Result<()> {
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
