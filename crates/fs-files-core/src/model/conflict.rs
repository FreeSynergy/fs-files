//! What to do when the target name is taken.
//!
//! Pattern: Strategy (value) — the caller chooses the rule; the default refuses. There is no
//! "replace": nothing here ever overwrites a file, with or without a flag.

/// The rule for a name that is already taken (`OnConflict` in the contract).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnConflict {
    /// Fail with `Conflict` and change nothing (the default).
    #[default]
    Refuse,
    /// Keep both: the incoming file or folder gets a free name, `name (2).ext`, `name (3).ext`, ...
    /// (a copy gets `name (copy).ext`, `name (copy 2).ext`).
    KeepBoth,
}
