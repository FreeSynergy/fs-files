//! Pattern: Strategy (Windows: no console window and a process group of its own, so Ctrl-C in the window's console does not
//! reach the server).

use std::os::windows::process::CommandExt;
use std::process::Command;

use super::Detach;

/// `CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP`.
const FLAGS: u32 = 0x0800_0200;

pub(super) struct NewGroup;

impl Detach for NewGroup {
    fn apply(&self, command: &mut Command) {
        command.creation_flags(FLAGS);
    }
}
