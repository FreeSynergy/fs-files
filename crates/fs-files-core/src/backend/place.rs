//! Where a blob sits and how a backend names it: the vocabulary of backends that keep real names.
//!
//! Pattern: Value objects — [`Naming`] is the Strategy switch (by id or by name), [`Spot`] names one
//! of the three places a blob can be, [`Scanned`] is one directory entry seen by `scan`.

/// How a backend names what it stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Naming {
    /// Keys are opaque ids chosen by the program (S3). Moving or renaming a file is a database change only.
    ById,
    /// Keys are paths of real names (`Urlaub/Foto.jpg`) (local folder). The disk is changed in step with the database.
    ByName,
}

/// A place a blob can be, for [`BlobBackend::relocate`](crate::BlobBackend::relocate) and `discard`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Spot {
    /// The committed blob or folder at this key (a path of real names on a [`Naming::ByName`] backend).
    Path(String),
    /// The unfinished upload with this key. Never shows up as a user file.
    Pending(String),
    /// The trash entry of this id. Never shows up as a user file.
    Trash(String),
}

/// What a directory entry is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// Something the program does not take over (symlink, odd name, ...), with the reason.
    Skipped(String),
}

/// One entry of a directory, as seen by [`BlobBackend::scan`](crate::BlobBackend::scan).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scanned {
    /// Name inside the directory.
    pub name: String,
    /// What it is.
    pub kind: ScanKind,
    /// Bytes (files only, else 0).
    pub size: u64,
    /// Last change in nanoseconds since the Unix epoch (0 if unknown).
    pub modified: i64,
}
