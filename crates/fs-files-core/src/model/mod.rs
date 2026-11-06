//! The data types of the contract.
//!
//! Pattern: Value objects (declarations) — plain data, no behaviour beyond conversion from the store.

mod file;
mod folder;
mod page;

pub use file::File;
pub(crate) use file::FileRecord;
pub use folder::Folder;
pub(crate) use folder::FolderRecord;
pub use page::{BackendInfo, Cursor, Entry, FileFilter, Page};
