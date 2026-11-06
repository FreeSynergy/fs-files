//! Reading a file (`Blob`) and copying bytes between keys.
//!
//! Pattern: Proxy — a [`FileBlob`] stands in for the bytes: size and ranges, no path, no backend.
//! Copying streams chunk by chunk, so memory stays one chunk whatever the file size.

use std::sync::Arc;

use async_trait::async_trait;
use fs_store::ObjectId;

use super::handle::FileRepo;
use crate::backend::{BlobBackend, CHUNK};
use crate::error::{Error, Result};
use crate::model::FileRecord;
use crate::transfer::Blob;

/// A key for new bytes: two random hex digits as a folder (spreads files), then the id.
pub(super) fn new_key(id: ObjectId) -> String {
    let s = id.to_string();
    format!("{}/{s}", &s[s.len() - 2..])
}

/// The bytes of one file.
pub(super) struct FileBlob {
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
        self.backend.get(&self.key, offset, length).await
    }
}

impl FileRepo {
    pub(super) async fn do_open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        let o = self.live_file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        Ok(Arc::new(FileBlob {
            backend: self.backends.get(&r.backend)?,
            key: r.key,
            size: r.size,
            media_type: r.media_type,
        }))
    }

    /// Copy the bytes of `r` to a fresh key on the same backend; verifies the checksum.
    pub(super) async fn copy_bytes(&self, r: &FileRecord) -> Result<String> {
        let backend = self.backends.get(&r.backend)?;
        let key = new_key(ObjectId::new());
        let run = async {
            let chunk = u32::try_from(CHUNK).unwrap_or(u32::MAX);
            backend.put(&key, 0, &[]).await?;
            let mut offset = 0u64;
            while offset < r.size {
                let data = backend.get(&r.key, offset, chunk).await?;
                if data.is_empty() {
                    return Err(Error::Backend("source is shorter than recorded".into()));
                }
                backend.put(&key, offset, &data).await?;
                offset += data.len() as u64;
            }
            let info = backend.commit(&key).await?;
            if info.size != r.size || info.checksum != r.checksum {
                return Err(Error::Backend("the copy does not match its source".into()));
            }
            Ok(())
        };
        match run.await {
            Ok(()) => Ok(key),
            Err(e) => {
                let _ = backend.delete(&key).await;
                Err(e)
            }
        }
    }
}
