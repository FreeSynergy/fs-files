//! Where the files may live next to the program's own data.
//!
//! Pattern: Specification — one rule, checked before anything is opened: the folder the files live
//! in must not hold the data directory. Else `objects.db` (and its journal) would lie among the
//! files, a rescan would take the database for a file of the person, and `backend.add` below the
//! root could reach it.

use std::path::{Component, Path, PathBuf};

/// The path as the file system sees it, though not all of it exists yet: the longest existing part
/// is resolved (links and all), the rest is appended. A `..` is refused by the caller before: after a
/// part that is not there, it cannot be resolved without making that part.
fn resolved(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = path.to_path_buf();
    loop {
        if let Ok(real) = at.canonicalize() {
            return rest.iter().rev().fold(real, |out, part| out.join(part));
        }
        match (at.file_name().map(ToOwned::to_owned), at.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                at = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Refuse a `root` for the files that is, or lies above, the `data` directory.
///
/// # Errors
/// The text says which two places collide, or that a path has a `..`.
pub fn check_places(root: &Path, data: &Path) -> Result<(), String> {
    for path in [root, data] {
        if path.components().any(|c| c == Component::ParentDir) {
            return Err(format!("{} must not contain \"..\"", path.display()));
        }
    }
    let (root, data) = (resolved(root), resolved(data));
    if data.starts_with(&root) {
        return Err(format!(
            "the folder of the files ({}) holds the data directory ({}) and so objects.db; \
             choose another FS_FILES_ROOT or --data",
            root.display(),
            data.display()
        ));
    }
    Ok(())
}
