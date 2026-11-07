//! Blob keys and their safety rules.
//!
//! Pattern: Specification — the rules a key must meet, as a pure check on a `&str`; every backend validates a key through
//! [`check_key`] before it touches anything, so a hostile key cannot reach a path or an object name.

use crate::error::{Error, Result};
use crate::note::Note;

/// Longest key in bytes.
pub const MAX_KEY: usize = 512;
/// Longest segment (between two `/`) in bytes.
pub const MAX_SEGMENT: usize = 200;

/// The one folder a `ByName` backend keeps for itself (trash, unfinished uploads). No user file or folder may have this name.
pub const RESERVED_NAME: &str = ".fs-files";

/// Check a key and return its segments.
///
/// A key is `/`-separated segments of `A-Z a-z 0-9 . _ -`. A segment is never empty and never starts
/// with `.`, so `..`, absolute paths, NUL, backslashes, drive letters and hidden names are refused.
pub fn check_key(key: &str) -> Result<Vec<&str>> {
    let bad = |why: &str| Err(Error::InvalidKey(Note::from(format!("{why}: {key:?}")).because(why)));
    if key.is_empty() || key.len() > MAX_KEY {
        return bad("empty or too long");
    }
    let segments: Vec<&str> = key.split('/').collect();
    for s in &segments {
        if s.is_empty() || s.len() > MAX_SEGMENT {
            return bad("empty or too long segment");
        }
        if s.starts_with('.') {
            return bad("segment starts with a dot");
        }
        if !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return bad("character not allowed");
        }
    }
    Ok(segments)
}
