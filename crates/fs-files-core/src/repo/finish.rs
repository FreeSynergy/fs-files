//! Finishing an upload: verify, checksum, and only then create the file.
//!
//! Pattern: Template method — a fixed flow (pre-check, commit, compare size, detect type, re-check
//! under the lock, create object, drop session) with the backend doing the byte work. The size
//! check compares what the backend held with what it stored (two readings of the same bytes; it
//! catches a short or doubled write, not a corrupting disk). Bytes are kept until the record is
//! written, so a failure never loses an upload; the record remembers its session, so a session
//! whose deletion failed is recognised as stale and never touches the file.

use fs_objects::ObjectId;

use super::handle::FileRepo;
use super::names::numbered_name;
use super::session::SessionRecord;
use super::steps::{Disk, Step};
use crate::backend::hash_chunks;
use crate::backend::{BlobBackend, BlobInfo, Spot};
use crate::error::{Error, Result};
use crate::model::{File, FileRecord, OnConflict};

const SNIFF: u32 = 8192;

impl FileRepo {
    pub(super) async fn finish_upload(&self, id: ObjectId, s: &SessionRecord) -> Result<File> {
        // One finish per upload at a time. Whoever comes second finds the file of the first.
        let _one = self.uploads.lock(&id.to_string()).await;
        // Finish ran before (or its session deletion failed): hand out the file, drop the session.
        if let Some(o) = self.file_of_upload(id).await? {
            let file = File::of(&o)?;
            self.drop_session(id).await?;
            return Ok(file);
        }
        self.session_obj(id).await?;
        let backend = self.backends.get(&s.backend)?;
        // Cheap checks first: a taken name leaves the upload intact (unless the upload said keep both).
        self.require_folder(s.folder).await?;
        if !s.keep_both {
            self.require_free(s.folder, &s.name, None).await?;
        }
        let file = if Self::is_named(&*backend) {
            self.finish_named(&*backend, s, id).await?
        } else {
            self.finish_by_id(&*backend, s, id).await?
        };
        self.drop_session(id).await?;
        Ok(file)
    }

    /// Delete the session record. Under the global lock: a rescan reads the records and must not see them change.
    async fn drop_session(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        match self.objects.delete(id, &self.actor).await {
            Ok(_) | Err(fs_objects::Error::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// The name the file gets: the one asked for, or with keep-both the first free one.
    async fn final_name(&self, s: &SessionRecord) -> Result<String> {
        let on = if s.keep_both {
            OnConflict::KeepBoth
        } else {
            OnConflict::Refuse
        };
        self.resolve_name(s.folder, &s.name, None, on, false, None)
            .await
    }

    /// The bytes stay in the backend until the record is written: a failure after the commit (the
    /// database is down, the name was taken meanwhile) leaves the upload to be finished again. A
    /// retry finds the blob already committed and goes on from there.
    async fn finish_by_id(
        &self,
        backend: &dyn BlobBackend,
        s: &SessionRecord,
        upload: ObjectId,
    ) -> Result<File> {
        let received = backend.pending(&s.key).await?;
        let committed = if received == 0 {
            backend.stat(&s.key).await?
        } else {
            None
        };
        let info = if let Some(c) = committed {
            let checksum = hash_chunks(c.size, |o, l| backend.get(&s.key, o, l)).await?;
            BlobInfo { checksum, ..c }
        } else {
            let info = backend.commit(&s.key).await?;
            if info.size != received {
                let _ = backend.delete(&s.key).await;
                return Err(Error::Backend(format!(
                    "size check failed: {received} bytes received, {} stored",
                    info.size
                )));
            }
            info
        };
        let _w = self.lock.lock().await;
        let name = self.final_name(s).await?;
        self.create_from_blob(backend, s, (&name, &s.key), false, info, upload)
            .await
    }

    /// Under the global lock: publish the pending bytes at the file's real path (atomic, never
    /// replacing), then write the record. If the record cannot be written the bytes go back to
    /// pending, so the upload can be finished again. With keep-both a name that is taken (in the
    /// records or on the disk) is skipped for the next free one.
    async fn finish_named(
        &self,
        backend: &dyn BlobBackend,
        s: &SessionRecord,
        upload: ObjectId,
    ) -> Result<File> {
        let _w = self.lock.lock().await;
        self.require_folder(s.folder).await?;
        let received = backend.pending(&s.key).await?;
        let mut tried = 0;
        let (name, key, info) = loop {
            tried += 1;
            let name = if tried == 1 {
                s.name.clone()
            } else {
                numbered_name(&s.name, tried)
            };
            let attempt = async {
                self.require_free(s.folder, &name, None).await?;
                let key = self.path_in(s.folder, &name).await?;
                let info = backend.publish(&s.key, &key).await?;
                Ok::<_, Error>((key, info))
            };
            match attempt.await {
                Ok((key, info)) => break (name, key, info),
                Err(Error::Conflict(_)) if s.keep_both && tried < 1000 => {}
                Err(e) => return Err(e),
            }
        };
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
        let db = self.create_from_blob(backend, s, (&name, &key), true, info, upload);
        Self::then_db(&[undo], db).await
    }

    /// Create the record. The caller holds the global lock.
    async fn create_from_blob(
        &self,
        backend: &dyn BlobBackend,
        s: &SessionRecord,
        (name, key): (&str, &str),
        named: bool,
        info: BlobInfo,
        upload: ObjectId,
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
            modified: (info.modified != 0).then_some(info.modified),
            upload: Some(upload.to_string()),
        };
        self.require_folder(s.folder).await?; // the world may have changed during the upload
        self.require_free(s.folder, name, None).await?;
        let o = self.file_object(name, &record);
        self.save_file(o).await
    }
}
