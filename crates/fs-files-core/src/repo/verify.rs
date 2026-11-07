//! Are the bytes at a place the ones the record is about?
//!
//! Pattern: Specification — a record says "these bytes: this size, this checksum, last changed
//! then". Before the program moves, trashes or deletes bytes on a disk people can change, it checks
//! that they still are those bytes: size first, then the modification time, and on doubt the
//! checksum. Anything else is somebody else's file, and nothing is touched ("changed outside").

use super::handle::FileRepo;
use crate::backend::{BlobBackend, Spot, hash_chunks};
use crate::error::{Error, Result};
use crate::model::FileRecord;

const READ: u32 = 1 << 20;
/// Files up to this size are always checked by checksum before a destructive step (256 MiB).
pub(super) const ALWAYS_HASH: u64 = 256 << 20;

/// The error for bytes that are not the recorded ones.
pub(super) fn changed(why: &str) -> Error {
    Error::Backend(format!("changed outside: {why}; run rescan"))
}

/// `blake3:<hex>` of `size` bytes at `key`, read in chunks.
pub(super) async fn checksum_at(backend: &dyn BlobBackend, key: &str, size: u64) -> Result<String> {
    hash_chunks(size, |o, l| backend.get(key, o, l.min(READ))).await
}

/// What lies in the trash folder under a file's id.
pub(super) enum InTrash {
    /// Nothing.
    Absent,
    /// The recorded bytes (size and checksum agree).
    Ours,
    /// Something else: not ours, never to be moved or deleted by a record.
    Stray,
}

impl FileRepo {
    /// Look into the trash folder for the file `id` and compare with its record.
    pub(super) async fn in_trash(
        &self,
        backend: &dyn BlobBackend,
        id: fs_objects::ObjectId,
        r: &FileRecord,
    ) -> Result<InTrash> {
        Ok(match backend.inspect(&Spot::Trash(id.to_string())).await? {
            None => InTrash::Absent,
            Some(i) if i.size == r.size && i.checksum == r.checksum => InTrash::Ours,
            Some(_) => InTrash::Stray,
        })
    }

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
        // Before anything is moved or deleted (`deep`) the checksum always decides, up to `ALWAYS_HASH`
        // bytes: a stranger's file of the same size whose modification time was copied (`touch -r`) must
        // not pass. Above that size the time is trusted (reading a file of that size on every trash
        // would be too slow); DESIGN states the limit.
        let same_time = info.modified != 0 && r.modified == Some(info.modified); // no or epoch time: changed
        if same_time && !(deep && info.size <= ALWAYS_HASH) {
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
