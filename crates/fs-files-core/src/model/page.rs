//! Entries, pages, cursors and the search filter.
//!
//! Pattern: Iterator-style paging — a [`Page`] carries a [`Cursor`] to ask for the next one;
//! the cursor is opaque to callers.

use fs_store::ObjectId;

use super::{File, Folder};

/// A file or a folder (`Entry` in `files.capnp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A file.
    File(File),
    /// A folder.
    Folder(Folder),
}

/// Where the next page starts. Opaque: only pass back what a [`Page`] gave you.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor(pub(crate) usize);

/// One page of entries (`EntryPage`); `next` is `None` on the last page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The entries, in name order.
    pub items: Vec<Entry>,
    /// Cursor of the next page.
    pub next: Option<Cursor>,
}

/// A point in time.
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// Search filter (`FileFilter`); all members optional, combined with AND.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    /// Substring of the name, case-insensitive.
    pub name: Option<String>,
    /// Prefix of the media type, for example `image/`.
    pub media_type: Option<String>,
    /// Only entries of this folder.
    pub folder: Option<ObjectId>,
    /// At least this many bytes (a folder counts as 0).
    pub min_size: Option<u64>,
    /// At most this many bytes (a folder counts as 0).
    pub max_size: Option<u64>,
    /// Changed at or after this time (the record's last change, see `SortBy::Modified`).
    pub modified_after: Option<Timestamp>,
    /// Changed before this time.
    pub modified_before: Option<Timestamp>,
    /// Created (or taken over by a rescan) at or after this time.
    pub created_after: Option<Timestamp>,
    /// Created before this time.
    pub created_before: Option<Timestamp>,
    /// Look in the trash instead of the live files. Not in the contract (it has no way to see the
    /// trash); reported to the owner.
    pub trashed: bool,
}

/// What `Backends::list` reports (`BackendInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendInfo {
    /// The id files record.
    pub id: String,
    /// `local`, `s3`, ...
    pub kind: String,
    /// New files go here.
    pub active: bool,
}
