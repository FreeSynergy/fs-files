//! Pattern: Strategy (Unix: a process group of its own, so a signal to the window's group does not reach the server).

use std::os::unix::process::CommandExt;
use std::process::Command;

use super::Detach;

pub(super) struct ProcessGroup;

impl Detach for ProcessGroup {
    fn apply(&self, command: &mut Command) {
        command.process_group(0);
    }
}
