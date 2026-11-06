//! Copying a file: stream the bytes, give the copy a free name, then publish it.
//!
//! Pattern: Template method — copy into a pending blob (memory: one chunk), then either commit by id
//! (S3) or, under the lock, pick a free name and `publish` at the real path (never replacing), then
//! write the record; if that fails the copy is moved back to pending and removed.

use fs_store::{ObjectId, StoredObject};

use super::handle::FileRepo;
use super::names::copy_name;
use super::steps::{Disk, Step};
use crate::backend::{BlobInfo, Spot};
use crate::error::{Error, Result};
use crate::model::{File, FileRecord};

impl FileRepo {
    pub(super) async fn do_copy_file(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
    ) -> Result<File> {
        let src = self.live_file_obj(id).await?;
        self.require_folder(folder).await?;
        let src_rec = FileRecord::of(&src)?;
        let backend = self.backends.get(&src_rec.backend)?;
        let src_key = self.key_of(&src, &src_rec).await?;
        let tmp = self.copy_bytes(&*backend, &src_key, &src_rec).await?;
        let named = Self::is_named(&*backend);
        let committed = !named;
        if committed {
            self.commit_copy(&*backend, &tmp, &src_rec).await?;
        }
        let _w = self.lock.lock().await;
        let outcome = self
            .register_copy(&src, &src_rec, folder, &tmp, &*backend)
            .await;
        if outcome.is_err() && committed {
            let _ = backend.delete(&tmp).await;
        } else if outcome.is_err() {
            let _ = backend.discard(&Spot::Pending(tmp)).await;
        }
        outcome
    }

    /// The copy is complete in the backend; give it a free name and a record.
    async fn register_copy(
        &self,
        src: &StoredObject,
        src_rec: &FileRecord,
        folder: Option<ObjectId>,
        tmp: &str,
        backend: &dyn crate::backend::BlobBackend,
    ) -> Result<File> {
        let name = self.free_copy_name(folder, &src.envelope.name).await?;
        let mut r = src_rec.clone();
        r.folder = folder;
        r.modified = None;
        let mut undo = None;
        if Self::is_named(backend) {
            let key = self.path_in(folder, &name).await?;
            let info = backend.publish(tmp, &key).await?;
            r.named = true;
            r.key = String::new();
            // what publish did; its undo moves the copy back to pending
            let published = Disk::Move(Spot::Pending(tmp.to_string()), Spot::Path(key));
            let step = Step::new(self.backends.get(&r.backend)?, published, false);
            if let Err(e) = Self::same_bytes(&info, src_rec) {
                step.undo().await;
                return Err(e);
            }
            undo = Some(step);
        } else {
            r.key = tmp.to_string();
        }
        let mut o = self.file_object(&name, &r);
        o.envelope.sensitive.clone_from(&src.envelope.sensitive);
        let file = File::of(&o)?;
        let db = self.save(o);
        match undo {
            Some(step) => Self::then_db(&[step], db).await?,
            None => db.await?,
        }
        Ok(file)
    }

    pub(super) fn same_bytes(info: &BlobInfo, r: &FileRecord) -> Result<()> {
        if info.size == r.size && info.checksum == r.checksum {
            Ok(())
        } else {
            Err(Error::Backend("the copy does not match its source".into()))
        }
    }

    async fn free_copy_name(&self, folder: Option<ObjectId>, name: &str) -> Result<String> {
        if self.require_free(folder, name, None).await.is_ok() {
            return Ok(name.to_string());
        }
        for n in 1..1000 {
            let candidate = copy_name(name, n);
            if self.require_free(folder, &candidate, None).await.is_ok() {
                return Ok(candidate);
            }
        }
        Err(Error::Conflict("no free name for the copy".into()))
    }
}
