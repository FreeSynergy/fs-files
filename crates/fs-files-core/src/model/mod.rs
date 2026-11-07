//! The data types of the contract.
//!
//! Pattern: Value objects (declarations) — plain data, no behaviour beyond conversion from the store.

mod conflict;
mod file;
mod folder;
mod page;
mod rescan;
mod sort;
mod upload;

pub use conflict::OnConflict;
pub use file::File;
pub(crate) use file::FileRecord;
pub use folder::Folder;
pub(crate) use folder::FolderRecord;
pub use page::{BackendInfo, Cursor, Entry, FileFilter, Page};
pub use rescan::{RescanReport, TrashReport};
pub use sort::{Sort, SortBy};
pub use upload::{UploadInfo, UploadPage};
