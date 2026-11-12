//! Sort order of a listing or a search.
//!
//! Pattern: Value object — what the caller asks for; the store does the ordering (index fields).

/// What to sort by (`SortBy` in the contract).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortBy {
    /// By name (case-insensitive, Unicode-normalised). The default.
    #[default]
    Name,
    /// By the time the record last changed (rename, move, new bytes found by rescan, ...).
    Modified,
    /// By size; a folder counts as 0.
    Size,
    /// By media type, ascending only; a folder counts as `inode/directory`.
    Type,
}

/// A sort order (`Sort` in the contract). Ties are broken by name, then id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sort {
    /// The key.
    pub by: SortBy,
    /// Largest, newest or last first.
    pub descending: bool,
    /// Folders come before files, each group in this order (the order of a file manager). Off by default
    /// in the core; the server sets it from `Sort.foldersFirst` of the contract (fs-interfaces v0.0.19, default true).
    pub folders_first: bool,
}

impl Sort {
    /// This key and direction, folders and files mixed.
    pub fn new(by: SortBy, descending: bool) -> Self {
        Self {
            by,
            descending,
            folders_first: false,
        }
    }

    /// The same order with the folders first.
    #[must_use]
    pub fn with_folders_first(mut self) -> Self {
        self.folders_first = true;
        self
    }
}
