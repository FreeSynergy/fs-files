//! BLAKE3 checksums, computed while the bytes stream in.
//!
//! Pattern: Strategy (the hash) + Memento-free running state — a backend feeds every chunk to
//! [`Digests`]; at commit the finished hash is used if it saw every byte in order, otherwise (after
//! a restart) the stored bytes are streamed once through [`hash_chunks`]. Memory stays one chunk.

use std::collections::HashMap;
use std::sync::Mutex;

/// Chunk size used for streaming reads (1 MiB).
pub const CHUNK: usize = 1 << 20;

/// The `blake3:<hex>` form of a checksum (the contract's `File.checksum`).
pub fn format_checksum(hash: &blake3::Hash) -> String {
    format!("blake3:{}", hash.to_hex())
}

/// Running hashes of the uploads this process has seen from byte 0 on.
#[derive(Default)]
pub(crate) struct Digests(Mutex<HashMap<String, (blake3::Hasher, u64)>>);

impl Digests {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, (blake3::Hasher, u64)>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Feed a chunk written at `offset`. Out-of-order or resumed chunks drop the running hash.
    pub(crate) fn feed(&self, key: &str, offset: u64, data: &[u8]) {
        let mut map = self.map();
        if offset == 0 {
            map.insert(key.to_string(), (blake3::Hasher::new(), 0));
        }
        let Some((hasher, len)) = map.get_mut(key) else {
            return;
        };
        if *len == offset {
            hasher.update(data);
            *len += data.len() as u64;
        } else {
            map.remove(key);
        }
    }

    /// The checksum, if the running hash saw exactly `size` bytes. Consumes it.
    pub(crate) fn finish(&self, key: &str, size: u64) -> Option<String> {
        let (hasher, len) = self.map().remove(key)?;
        (len == size).then(|| format_checksum(&hasher.finalize()))
    }

    /// Forget a key.
    pub(crate) fn forget(&self, key: &str) {
        self.map().remove(key);
    }
}

/// Hash `size` bytes by asking `read(offset, length)` for chunks; memory stays one chunk.
pub(crate) async fn hash_chunks<F, Fut>(size: u64, mut read: F) -> crate::Result<String>
where
    F: FnMut(u64, u32) -> Fut,
    Fut: Future<Output = crate::Result<Vec<u8>>>,
{
    let mut hasher = blake3::Hasher::new();
    let mut offset = 0;
    while offset < size {
        let want = u32::try_from((size - offset).min(CHUNK as u64)).unwrap_or(u32::MAX);
        let data = read(offset, want).await?;
        if data.is_empty() {
            return Err(crate::Error::Backend(
                "blob is shorter than its size".into(),
            ));
        }
        hasher.update(&data);
        offset += data.len() as u64;
    }
    Ok(format_checksum(&hasher.finalize()))
}
