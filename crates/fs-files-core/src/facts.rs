//! [`Facts`]: what an [`Error`] knows, typed.
//!
//! Pattern: none (a function that only computes from its argument). The server states these as
//! problem extensions; a consumer reads them instead of cutting them out of the message.

use crate::error::Error;
use crate::note::{Kind, Note, Subject};

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

impl Error {
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
                fs_objects::Error::NotFound(id) | fs_objects::Error::EncryptionRequired(id),
            ) => facts.id = Some(id.to_string()),
            Self::Store(fs_objects::Error::Invalid(why)) => facts.issues.push(why.clone()),
            Self::Json(e) | Self::Store(fs_objects::Error::Serde(e)) => {
                facts.issues.push(e.to_string());
            }
            Self::Io(e) if e.raw_os_error() == Some(36) => facts.issues.push("too-long".into()),
            _ => {}
        }
        facts
    }
}
