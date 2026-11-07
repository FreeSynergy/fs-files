//! Error type of the files core.
//!
//! Pattern: none (plain error enum via `thiserror`) — one `Error` for every public operation, with
//! `From` conversions for the store, IO and JSON layers. The facts a consumer needs (which object,
//! which name, which reasons) are in the [`Note`] of a variant and come out through [`Error::facts`].

use crate::note::{Kind, Note, Subject};

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
    NotFound(Note),
    /// A file or folder name is not acceptable.
    #[error("invalid name: {0}")]
    InvalidName(Note),
    /// A blob key is not acceptable (path safety).
    #[error("invalid key: {0}")]
    InvalidKey(Note),
    /// The operation conflicts with the current state (name taken, folder not empty, cycle, ...).
    #[error("{0}")]
    Conflict(Note),
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
    Backend(Note),
    /// A backend configuration is unusable.
    #[error("configuration: {0}")]
    Config(Note),
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

/// The typed facts of an error ([`Error::facts`]). The server states them as problem extensions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The id of the object the error is about.
    pub id: Option<String>,
    /// What that object is.
    pub kind: Option<Kind>,
    /// The name that is taken or not acceptable.
    pub name: Option<String>,
    /// Why a value was refused, one entry per reason.
    pub issues: Vec<String>,
    /// For a wrong offset: the bytes the backend holds and the offset the caller asked for.
    pub offsets: Option<(u64, u64)>,
}

impl Facts {
    fn take(&mut self, n: &Note) {
        if let Some(Subject { kind, id }) = &n.subject {
            self.id = Some(id.clone());
            self.kind = Some(*kind);
        }
        self.name.clone_from(&n.name);
        self.issues.clone_from(&n.issues);
    }
}

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
        Self::Conflict(format!("{NAME_TAKEN}{}", detail.into()).into())
    }

    /// A name clash on `name`.
    pub(crate) fn name_taken_as(name: &str, detail: impl Into<String>) -> Self {
        Self::Conflict(Note::from(format!("{NAME_TAKEN}{}", detail.into())).named(name))
    }

    /// Nothing of this `kind` has this `id`.
    pub(crate) fn missing(kind: Kind, id: impl ToString + std::fmt::Display) -> Self {
        let text = format!("{} {id}", kind.noun());
        Self::NotFound(Note::from(text).about(kind, id))
    }

    /// A name that is not acceptable, and why.
    pub(crate) fn bad_name(name: &str, why: &str) -> Self {
        Self::InvalidName(Note::from(format!("{why}: {name:?}")).named(name).because(why))
    }

    /// This error about `kind` `id`, if it has no subject yet and is one that can name one.
    #[must_use]
    pub(crate) fn about(mut self, kind: Kind, id: impl ToString) -> Self {
        if let Self::NotFound(n)
        | Self::Conflict(n)
        | Self::InvalidName(n)
        | Self::InvalidKey(n)
        | Self::Backend(n) = &mut self
        {
            *n = std::mem::take(n).about(kind, id);
        }
        self
    }

    /// The facts of this error, typed: what a consumer used to cut out of the message.
    pub fn facts(&self) -> Facts {
        let mut facts = Facts::default();
        match self {
            Self::NotFound(n)
            | Self::Conflict(n)
            | Self::InvalidName(n)
            | Self::InvalidKey(n)
            | Self::Backend(n) => facts.take(n),
            // The reason of a refused configuration is its text.
            Self::Config(n) => {
                facts.take(n);
                if facts.issues.is_empty() {
                    facts.issues.push(n.text.clone());
                }
            }
            Self::Offset { expected, got } => facts.offsets = Some((*expected, *got)),
            Self::Store(
                fs_store::Error::NotFound(id) | fs_store::Error::EncryptionRequired(id),
            ) => facts.id = Some(id.to_string()),
            Self::Store(fs_store::Error::Invalid(why)) => facts.issues.push(why.clone()),
            Self::Json(e) | Self::Store(fs_store::Error::Serde(e)) => {
                facts.issues.push(e.to_string());
            }
            Self::Io(e) if e.raw_os_error() == Some(36) => facts.issues.push("too-long".into()),
            _ => {}
        }
        facts
    }

    /// The named problem of the contract this error maps to, if it has one: name clash, invalid
    /// name, changed outside. Other errors (not found, offset, store) have none.
    pub fn problem(&self) -> Option<Problem> {
        use fs_interfaces::files_capnp::{
            CHANGED_OUTSIDE_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE,
        };
        let (kind, title, status, detail) = match self {
            Self::Conflict(m) if m.text.starts_with(NAME_TAKEN) => (
                NAME_CONFLICT_PROBLEM_TYPE,
                "The name is taken",
                409,
                m.text.clone(),
            ),
            Self::InvalidKey(m) if m.text.contains("symlink") => (
                CHANGED_OUTSIDE_PROBLEM_TYPE,
                "The file was changed outside the program",
                409,
                self.to_string(),
            ),
            Self::InvalidName(_) | Self::InvalidKey(_) => (
                INVALID_NAME_PROBLEM_TYPE,
                "The name is not acceptable",
                422,
                self.to_string(),
            ),
            Self::Backend(m) | Self::NotFound(m) if m.text.starts_with(CHANGED_OUTSIDE) => (
                CHANGED_OUTSIDE_PROBLEM_TYPE,
                "The file was changed outside the program",
                409,
                m.text.clone(),
            ),
            Self::Io(e) if e.raw_os_error() == Some(36) => (
                INVALID_NAME_PROBLEM_TYPE,
                "The name is not acceptable",
                422,
                self.to_string(),
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
