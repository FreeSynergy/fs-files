//! Reading a file (`Blob`) and copying bytes between keys.
//!
//! Pattern: Adapter — a [`FileBlob`] presents the bytes under one key of a backend as the `Blob` trait: size and ranges, no path, no backend.
//! Copying streams chunk by chunk, so memory stays one chunk whatever the file size.

use std::sync::Arc;

use async_trait::async_trait;
use fs_store::ObjectId;

use super::handle::FileRepo;
use crate::backend::{BlobBackend, CHUNK, Spot};
use crate::error::{Error, Result};
use crate::model::FileRecord;
use crate::note::{Kind, Note};
use crate::transfer::Blob;

/// A key for new bytes: two random hex digits as a folder (spreads files), then the id.
pub(super) fn new_key(id: ObjectId) -> String {
    let s = id.to_string();
    format!("{}/{s}", &s[s.len() - 2..])
}

/// The bytes of one file.
pub(super) struct FileBlob {
    id: ObjectId,
    backend: Arc<dyn BlobBackend>,
    key: String,
    size: u64,
    media_type: String,
}

#[async_trait]
impl Blob for FileBlob {
    async fn size(&self) -> Result<(u64, String)> {
        Ok((self.size, self.media_type.clone()))
    }

    async fn read(&self, offset: u64, length: u32) -> Result<Vec<u8>> {
        if offset >= self.size {
            return Ok(Vec::new());
        }
        let left = self.size - offset;
        let length = u32::try_from(left.min(u64::from(length))).unwrap_or(length);
        let data = self.backend.get(&self.key, offset, length).await?;
        if data.len() < length as usize {
            return Err(Error::Backend(
                Note::from(
                    "changed outside: the file is shorter than recorded; it was changed outside the program (run rescan)",
                )
                .about(Kind::File, self.id),
            ));
        }
        Ok(data)
    }
}

impl FileRepo {
    pub(super) async fn do_open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        let o = self.live_file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        let backend = self.backends.get(&r.backend)?;
        let key = self.key_of(&o, &r).await?;
        if r.named {
            // The folder can be changed from outside: never hand out bytes that are not the recorded ones.
            // Cheap check only (size, modification time); `rescan` brings the record up to date.
            self.verify_bytes(&*backend, &key, &o, &r, false).await?;
        }
        Ok(Arc::new(FileBlob {
            id,
            backend,
            key,
            size: r.size,
            media_type: r.media_type,
        }))
    }

    /// Copy the bytes of `r` to a fresh key on the same backend; verifies the checksum.
    /// Stream the bytes of `src_key` into a new pending blob and return its key. Memory stays one chunk.
    /// The caller publishes or commits it; on failure here the pending blob is removed.
    pub(super) async fn copy_bytes(
        &self,
        backend: &dyn BlobBackend,
        src_key: &str,
        r: &FileRecord,
    ) -> Result<String> {
        let key = new_key(ObjectId::new());
        let run = async {
            let chunk = u32::try_from(CHUNK).unwrap_or(u32::MAX);
            backend.put(&key, 0, &[]).await?;
            let mut offset = 0u64;
            while offset < r.size {
                let data = backend.get(src_key, offset, chunk).await?;
                if data.is_empty() {
                    return Err(Error::Backend("source is shorter than recorded".into()));
                }
                backend.put(&key, offset, &data).await?;
                offset += data.len() as u64;
            }
            Ok(())
        };
        match run.await {
            Ok(()) => Ok(key),
            Err(e) => {
                let _ = backend.discard(&Spot::Pending(key)).await;
                Err(e)
            }
        }
    }

    /// Make a copy readable by id (backends that name by id) and check it against its source.
    pub(super) async fn commit_copy(
        &self,
        backend: &dyn BlobBackend,
        key: &str,
        r: &FileRecord,
    ) -> Result<()> {
        let checked = match backend.commit(key).await {
            Ok(info) => Self::same_bytes(&info, r),
            Err(e) => Err(e),
        };
        if checked.is_err() {
            let _ = backend.delete(key).await;
        }
        checked
    }
}
