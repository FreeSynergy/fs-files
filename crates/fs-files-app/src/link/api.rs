//! Pattern: Strategy (a `file:` link names a path the way the system writes paths: `file:///home/me/a.jpg` on Unix,
//! `file:///C:/Users/me/a.jpg` on Windows, where the drive letter follows the first slash).

use std::path::PathBuf;

/// Reading the path of a `file:` link.
pub trait FileLinks {
    /// The path for the (already percent-decoded) text after `file://` and the host, which starts with `/`.
    fn path_of(&self, text: &str) -> Option<PathBuf>;
}

/// The way of this system.
#[must_use]
pub fn current() -> &'static dyn FileLinks {
    #[cfg(unix)]
    return &super::unix::PlainPaths;
    #[cfg(windows)]
    return &super::windows::DrivePaths;
}
