//! [`Note`]: the message of an error and the facts a consumer would otherwise cut out of it.
//!
//! Pattern: Value object — small, immutable once built, compared by value. The text is what a person
//! reads (it never changes for a fact); the facts are typed: which object, which name, which reasons.
//! They are filled where the error is made, so nothing is parsed out of a sentence (`coding.md` §2a).

use std::fmt;

/// What kind of thing an error is about (the contract's noun, `kind` of a problem).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A file.
    File,
    /// A folder.
    Folder,
    /// An unfinished upload.
    Upload,
    /// A backend (the id is its name, not a uuid).
    Backend,
}

impl Kind {
    /// The noun in lower case, as the problem extension `kind` says it.
    pub fn noun(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
            Self::Upload => "upload",
            Self::Backend => "backend",
        }
    }
}

/// The object an error is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    /// What it is.
    pub kind: Kind,
    /// Its id, as text.
    pub id: String,
}

/// A message with its facts. `From<String>` and `From<&str>` make a note without facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// What a person reads.
    pub text: String,
    /// The object the error is about, if one is known.
    pub subject: Option<Subject>,
    /// The name that is taken or not acceptable.
    pub name: Option<String>,
    /// Why a value was refused, one entry per reason.
    pub issues: Vec<String>,
}

impl Note {
    /// This note about `kind` `id`; a subject set before stays.
    #[must_use]
    pub fn about(mut self, kind: Kind, id: impl ToString) -> Self {
        self.subject.get_or_insert(Subject {
            kind,
            id: id.to_string(),
        });
        self
    }

    /// This note about the name `name`.
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// This note with one more reason.
    #[must_use]
    pub fn because(mut self, issue: impl Into<String>) -> Self {
        self.issues.push(issue.into());
        self
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<String> for Note {
    fn from(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }
}

impl From<&str> for Note {
    fn from(text: &str) -> Self {
        text.to_string().into()
    }
}
