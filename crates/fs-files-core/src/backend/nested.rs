//! Is there a backend below this folder?
//!
//! Pattern: Specification — a pure question about a folder: does a program folder (`.fs-files`) lie
//! anywhere below it? A backend that is not mounted (after a restart) is found only this way. The
//! walk is bounded in depth and in entries, and fails closed: a tree it cannot finish is answered
//! as "yes", so a huge folder is refused rather than trusted. Links are not followed.

use std::path::Path;

use super::RESERVED_NAME;

/// Deepest level looked at.
pub(crate) const MAX_DEPTH: usize = 16;
/// Most entries looked at in all.
pub(crate) const MAX_ENTRIES: usize = 10_000;

/// Whether a program folder lies below `dir` (the caller has looked at `dir` itself), or the tree is too big to say.
pub(crate) fn holds_backend(dir: &Path) -> bool {
    let (mut todo, mut seen) = (vec![(dir.to_path_buf(), 0usize)], 0usize);
    while let Some((at, depth)) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            return true;
        };
        for entry in entries {
            seen += 1;
            let Ok(entry) = entry else { return true };
            if seen > MAX_ENTRIES {
                return true;
            }
            let Ok(kind) = entry.file_type() else {
                return true;
            };
            if !kind.is_dir() {
                continue;
            }
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(RESERVED_NAME)
            {
                return true;
            }
            if depth + 1 > MAX_DEPTH {
                return true;
            }
            todo.push((entry.path(), depth + 1));
        }
    }
    false
}
