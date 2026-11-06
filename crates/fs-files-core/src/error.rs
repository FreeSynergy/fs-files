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

/// Prefix of the message of a `Conflict` that is about a name (see [`Error::problem`]).
const NAME_TAKEN: &str = "name taken: ";
/// Prefix of the message of an error that is about changes made outside the program.
const CHANGED_OUTSIDE: &str = "changed outside: ";

/// An error in the RFC 9457 shape of the contract (`Problem` in `common.capnp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// `Problem.type`: one of the `*_PROBLEM_TYPE` constants of `fs_interfaces::files_capnp`.
    pub kind: &'static str,
    /// Short, fixed summary.
    pub title: &'static str,
    /// HTTP-like status.
    pub status: u16,
    /// The message of the error.
    pub detail: String,
}

impl Error {
    /// A name clash: the name is taken in the folder or on the disk. Still a [`Error::Conflict`].
    pub(crate) fn name_taken(detail: impl Into<String>) -> Self {
        Self::Conflict(format!("{NAME_TAKEN}{}", detail.into()))
    }

    /// The named problem of the contract this error maps to, if it has one: name clash, invalid
    /// name, changed outside. Other errors (not found, offset, store) have none.
    pub fn problem(&self) -> Option<Problem> {
        use fs_interfaces::files_capnp::{
            CHANGED_OUTSIDE_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE,
        };
        let (kind, title, status, detail) = match self {
            Self::Conflict(m) if m.starts_with(NAME_TAKEN) => (
                NAME_CONFLICT_PROBLEM_TYPE,
                "The name is taken",
                409,
                m.clone(),
            ),
            Self::InvalidName(_) | Self::InvalidKey(_) => (
                INVALID_NAME_PROBLEM_TYPE,
                "The name is not acceptable",
                422,
                self.to_string(),
            ),
            Self::Backend(m) | Self::NotFound(m) if m.starts_with(CHANGED_OUTSIDE) => (
                CHANGED_OUTSIDE_PROBLEM_TYPE,
                "The file was changed outside the program",
                409,
                m.clone(),
            ),
            _ => return None,
        };
        Some(Problem {
            kind,
            title,
            status,
            detail,
        })
    }
}
