//! Finishing an upload: verify, checksum, and only then create the file.
//!
//! Pattern: Template method — a fixed flow (pre-check, commit, verify size, detect type, re-check
//! under the lock, create object, drop session) with the backend doing the byte work. Whatever
//! fails after the commit removes the bytes again, so no orphan is left behind.

use fs_store::ObjectId;

use super::handle::FileRepo;
use super::session::SessionRecord;
use super::steps::{Disk, Step};
use crate::backend::{BlobBackend, BlobInfo, Spot};
use crate::error::{Error, Result};
use crate::model::{File, FileRecord};

const SNIFF: u32 = 8192;

impl FileRepo {
    pub(super) async fn finish_upload(&self, id: ObjectId, s: &SessionRecord) -> Result<File> {
        self.session_obj(id).await?;
        let backend = self.backends.get(&s.backend)?;
        // Cheap checks first: a taken name leaves the upload intact.
        self.require_folder(s.folder).await?;
        self.require_free(s.folder, &s.name, None).await?;

        let file = if Self::is_named(&*backend) {
            self.finish_named(&*backend, s).await?
        } else {
            self.finish_by_id(&*backend, s).await?
        };
        match self.objects.delete(id, &self.actor).await {
            Ok(_) | Err(fs_store::Error::NotFound(_)) => Ok(file),
            Err(e) => Err(e.into()),
        }
    }

    async fn finish_by_id(&self, backend: &dyn BlobBackend, s: &SessionRecord) -> Result<File> {
        let received = backend.pending(&s.key).await?;
        let info = backend.commit(&s.key).await?;
        if info.size != received {
            let _ = backend.delete(&s.key).await;
            return Err(Error::Backend(format!(
                "size check failed: {received} bytes received, {} stored",
                info.size
            )));
        }
        let _w = self.lock.lock().await;
        let outcome = self.create_from_blob(backend, s, &s.key, false, info).await;
        if outcome.is_err() {
            let _ = backend.delete(&s.key).await;
        }
        outcome
    }

    /// Under the global lock: publish the pending bytes at the file's real path (atomic, never
    /// replacing), then write the record. If the record cannot be written the bytes go back to
    /// pending, so the upload can be finished again.
    async fn finish_named(&self, backend: &dyn BlobBackend, s: &SessionRecord) -> Result<File> {
        let _w = self.lock.lock().await;
        self.require_folder(s.folder).await?;
        self.require_free(s.folder, &s.name, None).await?;
        let received = backend.pending(&s.key).await?;
        let key = self.path_in(s.folder, &s.name).await?;
        let info = backend.publish(&s.key, &key).await?;
        let undo = Step::new(
            self.backends.get(&s.backend)?,
            Disk::Move(Spot::Pending(s.key.clone()), Spot::Path(key.clone())), // what publish did; undo moves it back
            false,
        );
        if info.size != received {
            undo.undo().await;
            return Err(Error::Backend(format!(
                "size check failed: {received} bytes received, {} stored",
                info.size
            )));
        }
        let db = self.create_from_blob(backend, s, &key, true, info);
        Self::then_db(&[undo], db).await
    }

    /// Create the record. The caller holds the global lock.
    async fn create_from_blob(
        &self,
        backend: &dyn BlobBackend,
        s: &SessionRecord,
        key: &str,
        named: bool,
        info: BlobInfo,
    ) -> Result<File> {
        let media_type = if s.media_type.is_empty() || s.media_type == "application/octet-stream" {
            let head = backend.get(key, 0, SNIFF).await?;
            infer::get(&head)
                .map_or("application/octet-stream", |t| t.mime_type())
                .to_string()
        } else {
            s.media_type.clone()
        };
        let record = FileRecord {
            media_type,
            size: info.size,
            checksum: info.checksum,
            folder: s.folder,
            trashed: false,
            backend: s.backend.clone(),
            key: if named {
                String::new()
            } else {
                key.to_string()
            },
            named,
            modified: None,
        };
        self.require_folder(s.folder).await?; // the world may have changed during the upload
        self.require_free(s.folder, &s.name, None).await?;
        let o = self.file_object(&s.name, &record);
        let file = File::of(&o)?;
        self.save(o).await?;
        Ok(file)
    }
}
