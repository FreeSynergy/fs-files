//! Pattern: Strategy (three parts of a disk that differ per system: which names it takes, how it makes a change durable,
//! how it moves something without replacing what is there).

use std::io;
use std::path::Path;

/// What names the disk takes beyond the rules every system shares (no empty name, no separator, no control character).
pub(crate) trait NameRules: Send + Sync {
    /// Why `name`, one segment of a path, is not a name on this disk; `None` when it is fine.
    fn refuse(&self, name: &str) -> Option<&'static str>;
}

/// Making a change to a folder durable, so that a rename or a new file that was reported is still there after a power loss.
pub(crate) trait Durability: Send + Sync {
    /// Flushes the folder entry of `dir` to disk, where the system has such a thing.
    ///
    /// # Errors
    /// `io::Error` when the disk refuses.
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
}

/// Moving without replacing.
pub(crate) trait Moves: Send + Sync {
    /// Moves `from` to `to` in one atomic step that never replaces what is at `to`, when the system has such a step
    /// (`renameat2(RENAME_NOREPLACE)` on Linux, if `native` allows it). `Ok(false)` when there is none here and the caller
    /// must use the slower way.
    ///
    /// # Errors
    /// `io::Error`, `AlreadyExists` when `to` is there.
    fn rename_new_native(&self, from: &Path, to: &Path, native: bool) -> io::Result<bool>;
}
