//! The answer of `rescan`.
//!
//! Pattern: Value object — what a rescan found, so a frontend can show it. Nothing in it is a
//! deletion: vanished files are only reported.

use super::{Entry, File};

/// What `rescan` did and found (`RescanReport` in the contract).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RescanReport {
    /// Files and folders that were on the disk but unknown; now taken over.
    pub added: Vec<Entry>,
    /// Files whose size or modification time changed outside the program; size and checksum are new.
    /// Also files whose bytes were found in the trash folder and put back (after a crash).
    pub updated: Vec<File>,
    /// Files with a record but no bytes at their place. Their records are **kept**.
    pub missing: Vec<File>,
    /// Entries that were not taken over, with the reason (symlinks, odd names, a name taken elsewhere).
    pub skipped: Vec<String>,
}

/// What `empty_trash` did (`TrashReport` in the contract).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrashReport {
    /// Files purged for good.
    pub purged: u32,
    /// Files that could not be purged (they stay in the trash; the rest was still purged).
    pub failed: Vec<fs_store::ObjectId>,
}
