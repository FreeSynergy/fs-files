//! Error type of the files core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation, with
//! `From` conversions for the store, IO and JSON layers.

/// Result of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong in the files core.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The object store failed (this includes the refusal of sensitive objects, ADR 0029).
    #[error("object store: {0}")]
    Store(#[from] fs_store::Error),
    /// The explicit "not possible" of a provider (ADR 0045).
    #[error(transparent)]
    Unsupported(#[from] fs_interfaces::Unsupported),
    /// No such file, folder, upload, backend or blob.
    #[error("not found: {0}")]
    NotFound(String),
    /// A file or folder name is not acceptable.
    #[error("invalid name: {0}")]
    InvalidName(String),
    /// A blob key is not acceptable (path safety).
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// The operation conflicts with the current state (name taken, folder not empty, cycle, ...).
    #[error("{0}")]
    Conflict(String),
    /// A chunk was written at the wrong offset; nothing was written.
    #[error("wrong offset: the upload holds {expected} bytes, the chunk starts at {got}")]
    Offset {
        /// Bytes the backend holds for this key.
        expected: u64,
        /// Offset the caller asked for.
        got: u64,
    },
    /// A backend failed (the message never contains credentials).
    #[error("backend: {0}")]
    Backend(String),
    /// A backend configuration is unusable.
    #[error("configuration: {0}")]
    Config(String),
    /// Local IO failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSON could not be read or written.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
