//! Pattern: Strategy (what makes a started process independent of the window: a process group on Unix; no console window and
//! a group of its own on Windows). The caller sets the arguments and the files, this only makes it independent.

use std::process::Command;

/// Makes a command start a process that outlives the one that starts it.
pub trait Detach {
    /// Changes `command` so that the process it starts is independent of this one.
    fn apply(&self, command: &mut Command);
}

/// The way of this system.
#[must_use]
pub fn current() -> &'static dyn Detach {
    #[cfg(unix)]
    return &super::unix::ProcessGroup;
    #[cfg(windows)]
    return &super::windows::NewGroup;
}
