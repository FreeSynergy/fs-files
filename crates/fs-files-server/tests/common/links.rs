//! A link planted where the program might follow it, the way each system makes one: a symlink on Unix; on Windows a junction
//! for a folder (it needs no right) and a symlink for a file (it needs the right to make symlinks, which the runner and an
//! administrator have).
#![allow(dead_code)]

use std::path::Path;

/// `link` is a link to the existing folder `target`.
pub fn link_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    junction::create(target, link).unwrap();
}

/// `link` is a link to the file `target`.
pub fn link_file(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(target, link).unwrap();
}
