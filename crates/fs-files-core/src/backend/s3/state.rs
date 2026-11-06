//! The state of an upload in progress, kept in the bucket itself.
//!
//! Pattern: Memento — what is needed to go on after a restart (the multipart id, the finished
//! parts with their etags, the small pieces not yet in a part) is stored as objects next to the
//! data and rebuilt by [`load`]. So an upload never depends on this process staying alive.

use serde::{Deserialize, Serialize};

use super::api::ObjectApi;
use super::layout::Layout;
use crate::error::{Error, Result};

/// One finished part of the multipart upload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PartRec {
    pub etag: String,
    pub size: u64,
}

/// What is persisted in the `state` object.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Saved {
    /// Multipart id; `None` until the first part is due (small files never need one).
    pub id: Option<String>,
    pub parts: Vec<PartRec>,
}

/// A small piece of the upload that is not in a part yet.
#[derive(Clone, Copy, Debug)]
pub(super) struct Piece {
    pub offset: u64,
    pub size: u64,
}

/// Everything known about one upload in progress.
#[derive(Debug, Default)]
pub(super) struct Pending {
    pub saved: Saved,
    pub tail: Vec<Piece>,
}

impl Pending {
    /// Bytes inside finished parts.
    pub(super) fn uploaded(&self) -> u64 {
        self.saved.parts.iter().map(|p| p.size).sum()
    }

    /// Bytes in tail pieces.
    pub(super) fn tail_len(&self) -> u64 {
        self.tail.iter().map(|p| p.size).sum()
    }

    /// Bytes held in total: the position the next chunk must start at.
    pub(super) fn received(&self) -> u64 {
        self.uploaded() + self.tail_len()
    }
}

/// Persist the part list (the one object that says "this upload exists").
pub(super) async fn save(
    api: &dyn ObjectApi,
    layout: &Layout,
    key: &str,
    saved: &Saved,
) -> Result<()> {
    api.put(&layout.state(key), serde_json::to_vec(saved)?)
        .await
}

/// Rebuild the state of `key` from the bucket; `None` if no upload is in progress.
pub(super) async fn load(
    api: &dyn ObjectApi,
    layout: &Layout,
    key: &str,
) -> Result<Option<Pending>> {
    let name = layout.state(key);
    let Some(size) = api.head(&name).await? else {
        return Ok(None);
    };
    let raw = api
        .get_range(&name, 0, size)
        .await?
        .ok_or_else(|| Error::Backend("upload state vanished".into()))?;
    let saved: Saved = serde_json::from_slice(&raw)?;
    let mut pending = Pending {
        saved,
        tail: Vec::new(),
    };
    let done = pending.uploaded();
    let mut found = api.list(&layout.tail_dir(key)).await?;
    found.sort_by(|a, b| a.key.cmp(&b.key)); // zero-padded offsets sort numerically
    let mut next = done;
    for entry in found {
        let offset: u64 = entry
            .key
            .rsplit('/')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(u64::MAX);
        if offset < done {
            api.delete(&entry.key).await?; // already inside a part: left over from a crash
        } else if offset == next {
            pending.tail.push(Piece {
                offset,
                size: entry.size,
            });
            next += entry.size;
        } // a gap or junk: ignored, the caller writes at `received`
    }
    Ok(Some(pending))
}
