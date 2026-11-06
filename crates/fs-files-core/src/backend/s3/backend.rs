//! `S3Backend`: bytes in an S3 bucket (RustFS and any other S3 store).
//!
//! Pattern: Strategy (a `BlobBackend`) — stage 2 provider. A resumable upload is a multipart upload,
//! but S3 refuses parts under 5 MiB (except the last) and callers send chunks of any size. So
//! chunks first land as small *tail pieces* (each durable on its own); once the pieces add up to
//! the part size they are joined into one part and deleted. Memory stays one part, never the file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use fs_interfaces::Capabilities;

use super::api::ObjectApi;
use super::layout::Layout;
use super::state::{self, Pending, Piece, Saved};
use crate::backend::digest::{Digests, hash_chunks};
use crate::backend::key::check_key;
use crate::backend::lock::KeyLocks;
use crate::backend::provider::{BlobBackend, BlobInfo};
use crate::error::{Error, Result};

/// S3's smallest part (except the last).
pub const MIN_PART_SIZE: u64 = 5 << 20;
/// S3's largest number of parts per object.
pub const MAX_PARTS: usize = 10_000;

/// Settings of an S3 backend.
#[derive(Clone, Debug)]
pub struct S3Options {
    /// Key prefix inside the bucket (a "folder"); empty for none.
    pub prefix: String,
    /// Bytes per part, at least 5 MiB. An object can be at most `MAX_PARTS` x this (8 MiB: about 78 GiB).
    pub part_size: u64,
}

impl Default for S3Options {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            part_size: 8 << 20,
        }
    }
}

/// Bytes in an S3-compatible bucket.
pub struct S3Backend {
    pub(super) api: Arc<dyn ObjectApi>,
    pub(super) layout: Layout,
    pub(super) part_size: u64,
    locks: KeyLocks,
    digests: Digests,
    cache: Mutex<HashMap<String, Pending>>,
}

impl S3Backend {
    /// A backend over `api` (see [`ObjectStoreApi`](super::ObjectStoreApi) for a real bucket).
    pub fn new(api: Arc<dyn ObjectApi>, options: &S3Options) -> Result<Self> {
        if options.part_size < MIN_PART_SIZE {
            return Err(Error::Config(
                "part_size must be at least 5 MiB (the S3 minimum)".into(),
            ));
        }
        let prefix = options.prefix.trim_matches('/');
        if !prefix.is_empty() {
            check_key(prefix)
                .map_err(|_| Error::Config("prefix has characters that are not allowed".into()))?;
        }
        Ok(Self {
            api,
            layout: Layout::new(prefix),
            part_size: options.part_size,
            locks: KeyLocks::default(),
            digests: Digests::default(),
            cache: Mutex::new(HashMap::new()),
        })
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, HashMap<String, Pending>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The state of `key`: from memory, else rebuilt from the bucket. Taken out; give it back with `keep`.
    async fn take(&self, key: &str) -> Result<Option<Pending>> {
        if let Some(p) = self.cache().remove(key) {
            return Ok(Some(p));
        }
        state::load(&*self.api, &self.layout, key).await
    }

    fn keep(&self, key: &str, p: Pending) {
        self.cache().insert(key.to_string(), p);
    }
}

#[async_trait]
impl BlobBackend for S3Backend {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("s3");
        c.supported("blob.read")
            .supported("blob.write")
            .supported("blob.delete");
        c
    }

    fn kind(&self) -> &'static str {
        "s3"
    }

    async fn get(&self, key: &str, offset: u64, length: u32) -> Result<Vec<u8>> {
        check_key(key)?;
        let name = self.layout.data(key);
        let size = self
            .api
            .head(&name)
            .await?
            .ok_or_else(|| Error::NotFound(format!("blob {key}")))?;
        if offset >= size {
            return Ok(Vec::new());
        }
        let end = size.min(offset + u64::from(length));
        self.api
            .get_range(&name, offset, end)
            .await?
            .ok_or_else(|| Error::NotFound(format!("blob {key}")))
    }

    async fn stat(&self, key: &str) -> Result<Option<BlobInfo>> {
        check_key(key)?;
        let size = self.api.head(&self.layout.data(key)).await?;
        Ok(size.map(|size| BlobInfo {
            size,
            checksum: String::new(),
        }))
    }

    async fn put(&self, key: &str, offset: u64, data: &[u8]) -> Result<()> {
        check_key(key)?;
        let _guard = self.locks.lock(key).await;
        let mut p = match self.take(key).await? {
            Some(p) => p,
            None if offset != 0 => {
                return Err(Error::Offset {
                    expected: 0,
                    got: offset,
                });
            }
            None => {
                state::save(&*self.api, &self.layout, key, &Saved::default()).await?;
                Pending::default()
            }
        };
        if p.received() != offset {
            let expected = p.received();
            self.keep(key, p);
            return Err(Error::Offset {
                expected,
                got: offset,
            });
        }
        if !data.is_empty() {
            self.api
                .put(&self.layout.tail(key, offset), data.to_vec())
                .await?;
            p.tail.push(Piece {
                offset,
                size: data.len() as u64,
            });
        }
        self.flush(key, &mut p, false).await?;
        self.digests.feed(key, offset, data);
        self.keep(key, p);
        Ok(())
    }

    async fn pending(&self, key: &str) -> Result<u64> {
        check_key(key)?;
        let _guard = self.locks.lock(key).await;
        let Some(p) = self.take(key).await? else {
            return Ok(0);
        };
        let received = p.received();
        self.keep(key, p);
        Ok(received)
    }

    async fn commit(&self, key: &str) -> Result<BlobInfo> {
        check_key(key)?;
        let _guard = self.locks.lock(key).await;
        let mut p = self
            .take(key)
            .await?
            .ok_or_else(|| Error::NotFound(format!("nothing pending for {key}")))?;
        let size = p.received();
        let target = self.layout.data(key);
        if let Some(id) = p.saved.id.clone() {
            self.flush(key, &mut p, true).await?; // the last part may be small
            let etags = p.saved.parts.iter().map(|x| x.etag.clone()).collect();
            self.api.complete(&target, &id, etags).await?;
        } else {
            let mut data = Vec::new(); // a file under one part: a plain put, no multipart at all
            for piece in &p.tail {
                let name = self.layout.tail(key, piece.offset);
                data.extend(
                    self.api
                        .get_range(&name, 0, piece.size)
                        .await?
                        .unwrap_or_default(),
                );
            }
            self.api.put(&target, data).await?;
        }
        self.clear(key).await?;
        let checksum = match self.digests.finish(key, size) {
            Some(c) => c,
            None => hash_chunks(size, |o, l| self.get(key, o, l)).await?,
        };
        Ok(BlobInfo { size, checksum })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        check_key(key)?;
        let _guard = self.locks.lock(key).await;
        self.digests.forget(key);
        if let Some(id) = self.take(key).await?.and_then(|p| p.saved.id) {
            self.api.abort(&self.layout.data(key), &id).await?;
        }
        self.clear(key).await?;
        self.api.delete(&self.layout.data(key)).await
    }
}
