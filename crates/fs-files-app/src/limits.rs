//! The time limits of the calls to the files server.
//!
//! Pattern: none — plain configuration data (`Limits`) and the words of a timeout.

use std::time::Duration;

use fs_admin_frontend::BackendError;
use fs_interfaces::rpc::client::{CALL_TIMEOUT, Fault};

/// The name of the server in the words of a timeout.
pub(crate) const SERVER: &str = "files";

/// What the caller waits on top of a limit for a worker that cannot run the job.
pub(crate) const GRACE: Duration = Duration::from_secs(2);

/// Commands that move bytes or walk a whole tree: they get [`Limits::long`], every other call [`Limits::call`].
pub(crate) const LONG_COMMANDS: &[&str] = &[
    "upload",
    "download",
    "copy",
    "move",
    "restore",
    "restoreTo",
    "purge",
    "emptyTrash",
    "rescan",
];

/// How long the window waits for the server before it says so (no call waits for ever; a window never freezes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The questions asked right after the handshake and the type descriptions: the window is not open yet, so short.
    pub start: Duration,
    /// Every other call: the default of fs-interfaces (`CALL_TIMEOUT`, 30 s).
    pub call: Duration,
    /// Transfer, copy, move, restore, purge, empty trash and rescan: they read or write file bytes or a whole folder
    /// tree (a large file over S3, a move between backends is a copy), so a quarter of an hour.
    pub long: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            start: Duration::from_secs(10),
            call: CALL_TIMEOUT,
            long: Duration::from_secs(15 * 60),
        }
    }
}

/// A call that ran out of time, as the person sees it. `BackendError::Handshake` is the one variant that carries a text
/// for the person and a "Try again" (which connects anew), and a server that does not answer needs exactly that.
pub(crate) fn timed_out(fault: &Fault) -> BackendError {
    BackendError::Handshake(fault.to_string())
}
