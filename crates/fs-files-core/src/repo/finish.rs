//! Finishing an upload: verify, checksum, and only then create the file.
//!
//! Pattern: Template method — a fixed flow (pre-check, commit, verify size, detect type, re-check
//! under the lock, create object, drop session) with the backend doing the byte work. Whatever
//! fails after the commit removes the bytes again, so no orphan is left behind.

use fs_store::ObjectId;

use super::handle::FileRepo;
use super::session::SessionRecord;
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

        let received = backend.pending(&s.key).await?;
        let info = backend.commit(&s.key).await?;
        if info.size != received {
            let _ = backend.delete(&s.key).await;
            return Err(Error::Backend(format!(
                "size check failed: {received} bytes received, {} stored",
                info.size
            )));
        }
        let outcome = self.create_from_blob(s, info.size, info.checksum).await;
        if outcome.is_err() {
            let _ = backend.delete(&s.key).await;
        }
        let file = outcome?;
        match self.objects.delete(id, &self.actor).await {
            Ok(_) | Err(fs_store::Error::NotFound(_)) => Ok(file),
            Err(e) => Err(e.into()),
        }
    }

    async fn create_from_blob(
        &self,
        s: &SessionRecord,
        size: u64,
        checksum: String,
    ) -> Result<File> {
        let backend = self.backends.get(&s.backend)?;
        let media_type = if s.media_type.is_empty() || s.media_type == "application/octet-stream" {
            let head = backend.get(&s.key, 0, SNIFF).await?;
            infer::get(&head)
                .map_or("application/octet-stream", |t| t.mime_type())
                .to_string()
        } else {
            s.media_type.clone()
        };
        let record = FileRecord {
            media_type,
            size,
            checksum,
            folder: s.folder,
            trashed: false,
            backend: s.backend.clone(),
            key: s.key.clone(),
        };
        let _w = self.lock.lock().await;
        self.require_folder(s.folder).await?; // the world may have changed during the upload
        self.require_free(s.folder, &s.name, None).await?;
        let o = self.file_object(&s.name, &record);
        let file = File::of(&o)?;
        self.save(o).await?;
        Ok(file)
    }
}
