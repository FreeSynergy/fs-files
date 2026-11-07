//! Constants of the files objects.
//!
//! Pattern: Constants — AS2 types, context and content types of this program in one place.

/// `context` of all objects of this program.
pub const CONTEXT: &str = "files";
/// AS2 type of a folder.
pub const FOLDER_TYPE: &str = "Collection";
/// Own type of an upload session in the store (AS2 has none).
pub const UPLOAD_TYPE: &str = "Upload";
pub(super) const FILE_CONTENT: &str = "application/vnd.freesynergy.file+json";
pub(super) const FOLDER_CONTENT: &str = "application/vnd.freesynergy.folder+json";
pub(super) const UPLOAD_CONTENT: &str = "application/vnd.freesynergy.upload+json";
/// Index value of the top level folder.
pub(super) const ROOT: &str = "root";
/// Entries per page unless changed with `with_page_size`.
pub(super) const PAGE: usize = 100;
