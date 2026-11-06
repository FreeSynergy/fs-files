//! Names of files and folders.
//!
//! Pattern: Specification — what a name may be. Two names are the same only if they are equal byte
//! for byte: the operating system decides the rest (on Linux `Foto.jpg` and `foto.jpg` are two files).

use crate::error::{Error, Result};

/// Longest name in bytes.
pub const MAX_NAME: usize = 255;

/// Check a name: not empty, at most 255 bytes, no `/` or `\`, no control characters, not `.` or `..`.
/// On a backend that keeps real names a name is also a file name on the disk, so `.fs-files` (the program's
/// own folder) is refused. Whether the disk accepts the rest is decided by the disk, and answered clearly.
pub fn check_name(name: &str) -> Result<()> {
    let bad = |why: &str| Err(Error::InvalidName(format!("{why}: {name:?}")));
    if name.trim().is_empty() || name.len() > MAX_NAME {
        return bad("empty or longer than 255 bytes");
    }
    if name == "." || name == ".." {
        return bad("reserved");
    }
    if name.eq_ignore_ascii_case(crate::backend::RESERVED_NAME) {
        return bad(
            "reserved: the program keeps its trash and unfinished uploads in a folder of this name",
        );
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
/// `name` for the `n`-th copy: `a.txt` -> `a (copy).txt`, `a (copy 2).txt`.
/// `name (n).ext` for a kept-both name (n >= 2).
pub(super) fn numbered_name(name: &str, n: u32) -> String {
    let tag = format!(" ({n})");
    match name.rfind('.') {
        Some(i) if i > 0 => format!("{}{tag}{}", &name[..i], &name[i..]),
        _ => format!("{name}{tag}"),
    }
}

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
