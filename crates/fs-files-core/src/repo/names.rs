//! Names of files and folders.
//!
//! Pattern: Specification — what a name may be, and the key under which two names count as the same
//! (case-insensitive, Unicode-normalised, through the store's `sort_key`).

use crate::error::{Error, Result};

/// Longest name in bytes.
pub const MAX_NAME: usize = 255;

/// Check a name: not empty, at most 255 bytes, no `/` or `\`, no control characters, not `.` or `..`.
/// Names are never used as paths, but they are shown and exported, so they must be harmless there too.
pub fn check_name(name: &str) -> Result<()> {
    let bad = |why: &str| Err(Error::InvalidName(format!("{why}: {name:?}")));
    if name.trim().is_empty() || name.len() > MAX_NAME {
        return bad("empty or longer than 255 bytes");
    }
    if name == "." || name == ".." {
        return bad("reserved");
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return bad("contains a separator or control character");
    }
    Ok(())
}

/// The comparison key of a name.
pub(super) fn name_key(name: &str) -> String {
    fs_store::sort_key(name)
}

/// `name` for the `n`-th copy: `a.txt` -> `a (copy).txt`, `a (copy 2).txt`.
pub(super) fn copy_name(name: &str, n: u32) -> String {
    let tag = if n <= 1 {
        " (copy)".to_string()
    } else {
        format!(" (copy {n})")
    };
    match name.rfind('.') {
        Some(i) if i > 0 => format!("{}{tag}{}", &name[..i], &name[i..]),
        _ => format!("{name}{tag}"),
    }
}
