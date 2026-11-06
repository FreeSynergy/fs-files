//! Are the bytes at a place the ones the record is about?
//!
//! Pattern: Specification — a record says "these bytes: this size, this checksum, last changed
//! then". Before the program moves, trashes or deletes bytes on a disk people can change, it checks
//! that they still are those bytes: size first, then the modification time, and on doubt the
//! checksum. Anything else is somebody else's file, and nothing is touched ("changed outside").

use super::handle::FileRepo;
use crate::backend::{BlobBackend, hash_chunks};
use crate::error::{Error, Result};
use crate::model::FileRecord;

const READ: u32 = 1 << 20;

/// The error for bytes that are not the recorded ones.
pub(super) fn changed(why: &str) -> Error {
    Error::Backend(format!("changed outside: {why}; run rescan"))
}

/// `blake3:<hex>` of `size` bytes at `key`, read in chunks.
pub(super) async fn checksum_at(backend: &dyn BlobBackend, key: &str, size: u64) -> Result<String> {
    hash_chunks(size, |o, l| backend.get(key, o, l.min(READ))).await
}

impl FileRepo {
    /// Check that `key` holds the bytes of `r`. `deep`: when the modification time is not the
    /// recorded one, read the bytes and compare the checksum (needed before anything is moved or
    /// deleted). Not deep (reading a file): size and modification time only, which is cheap.
    pub(super) async fn verify_bytes(
        &self,
        backend: &dyn BlobBackend,
        key: &str,
        name: &str,
        r: &FileRecord,
        deep: bool,
    ) -> Result<()> {
        let info = match backend.stat(key).await {
            Ok(Some(i)) => i,
            Ok(None) => return Err(changed(&format!("{name:?} is not on the disk any more"))),
            Err(Error::Backend(_)) => {
                return Err(changed(&format!("{name:?} is not a file any more")));
            }
            Err(e) => return Err(e),
        };
        if info.size != r.size {
            return Err(changed(&format!(
                "{name:?} has {} bytes, the record says {}",
                info.size, r.size
            )));
        }
        let same_time = r.modified == Some(info.modified);
        if same_time || info.modified == 0 {
            return Ok(());
        }
        if !deep {
            if r.modified.is_some() {
                return Err(changed(&format!("{name:?} was modified")));
            }
            return Ok(()); // never looked at by a rescan: size is all there is
        }
        if checksum_at(backend, key, info.size).await? == r.checksum {
            return Ok(());
        }
        Err(changed(&format!("{name:?} has other bytes than recorded")))
    }
}
