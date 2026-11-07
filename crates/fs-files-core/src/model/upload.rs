//! An unfinished upload, as listed by `pending_uploads`.
//!
//! Pattern: Value object.

use fs_store::ObjectId;

/// One unfinished upload (`UploadInfo` in the contract).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadInfo {
    /// The id to give to `resume_upload`.
    pub id: String,
    /// The folder the file will be in.
    pub folder: Option<ObjectId>,
    /// The name the file will have.
    pub name: String,
    /// The media type given at the start (may be empty).
    pub media_type: String,
    /// Bytes the backend holds so far (where `write` has to go on).
    pub received: u64,
    /// When the upload was started.
    pub started: chrono::DateTime<chrono::Utc>,
}

/// One page of unfinished uploads; `next` is `None` on the last page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadPage {
    /// The uploads of this page, oldest first.
    pub items: Vec<UploadInfo>,
    /// Where the next page starts.
    pub next: Option<super::Cursor>,
}
