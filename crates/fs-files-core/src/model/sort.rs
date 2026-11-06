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
}
