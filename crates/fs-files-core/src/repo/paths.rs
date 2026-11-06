//! Where a file or folder is on a disk that keeps real names.
//!
//! Pattern: Specification of the mapping — a path is never stored for a named file; it is derived from
//! the folder names and the file name when needed. So renaming or moving a folder is one directory
//! rename on the disk and one record in the database, and the files below follow by themselves.

use fs_store::{ObjectId, StoredObject};

use super::handle::FileRepo;
use crate::backend::{BlobBackend, Naming};
use crate::error::{Error, Result};
use crate::model::{FileRecord, FolderRecord};

/// Folders nested deeper than this are refused (also stops a damaged parent loop).
const MAX_DEPTH: usize = 512;

impl FileRepo {
    /// Names from the top down to `folder` (empty for the top level).
    pub(super) async fn folder_names(&self, folder: Option<ObjectId>) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let mut at = folder;
        while let Some(f) = at {
            let o = self.folder_obj(f).await?;
            names.push(o.envelope.name.clone());
            at = FolderRecord::of(&o)?.parent;
            if names.len() > MAX_DEPTH {
                return Err(Error::Conflict("folders are nested too deep".into()));
            }
        }
        names.reverse();
        Ok(names)
    }

    /// The path of an entry called `name` inside `folder`, as a backend key.
    pub(super) async fn path_in(&self, folder: Option<ObjectId>, name: &str) -> Result<String> {
        let mut names = self.folder_names(folder).await?;
        names.push(name.to_string());
        Ok(names.join("/"))
    }

    /// The key under which the bytes of a stored file are in its backend.
    pub(super) async fn key_of(&self, o: &StoredObject, r: &FileRecord) -> Result<String> {
        if r.named {
            self.path_in(r.folder, &o.envelope.name).await
        } else {
            Ok(r.key.clone())
        }
    }

    /// True if new files on this backend go under their real names.
    pub(super) fn is_named(backend: &dyn BlobBackend) -> bool {
        backend.naming() == Naming::ByName
    }

    /// Before an upload or a copy starts: the target must be a valid, free path on the disk.
    /// Nothing is created.
    pub(super) async fn check_target(
        &self,
        backend: &dyn BlobBackend,
        folder: Option<ObjectId>,
        name: &str,
    ) -> Result<()> {
        if !Self::is_named(backend) {
            return Ok(());
        }
        let key = self.path_in(folder, name).await?;
        match backend.stat(&key).await {
            Ok(None) => Ok(()),
            Ok(Some(_)) | Err(Error::Backend(_)) => Err(Error::name_taken(format!(
                "{name:?} exists on the disk but is not known here; run rescan to take it over"
            ))),
            Err(e) => Err(e),
        }
    }
}
