//! Turning tail pieces into parts, and cleaning up.
//!
//! Pattern: Template method — the fixed order that makes a crash harmless: send the part, save the
//! part list, only then delete the pieces. After a crash in between, the leftover pieces are
//! recognised by their offset (below what the parts already hold) and dropped on load.

use super::backend::{MAX_PARTS, S3Backend};
use super::state::{self, Pending};
use crate::error::{Error, Result};

impl S3Backend {
    /// Join tail pieces into parts while enough bytes are waiting; with `last`, everything left.
    pub(super) async fn flush(&self, key: &str, p: &mut Pending, last: bool) -> Result<()> {
        while !p.tail.is_empty() && (last || p.tail_len() >= self.part_size) {
            let (mut count, mut size) = (0, 0);
            for piece in &p.tail {
                count += 1;
                size += piece.size;
                if !last && size >= self.part_size {
                    break;
                }
            }
            let mut data = Vec::new();
            for piece in &p.tail[..count] {
                let name = self.layout.tail(key, piece.offset);
                let got = self.api.get_range(&name, 0, piece.size).await?;
                data.extend(got.ok_or_else(|| Error::Backend("a tail piece is missing".into()))?);
            }
            if p.saved.parts.len() >= MAX_PARTS {
                return Err(Error::Backend(
                    "too many parts; use a larger part_size for files this big".into(),
                ));
            }
            let target = self.layout.data(key);
            let id = if let Some(id) = &p.saved.id {
                id.clone()
            } else {
                let id = self.api.create_multipart(&target).await?;
                p.saved.id = Some(id.clone());
                state::save(&*self.api, &self.layout, key, &p.saved).await?;
                id
            };
            let etag = self
                .api
                .put_part(&target, &id, p.saved.parts.len(), data)
                .await?;
            p.saved.parts.push(state::PartRec { etag, size });
            state::save(&*self.api, &self.layout, key, &p.saved).await?; // the part is now safe...
            for piece in p.tail.drain(..count) {
                self.api
                    .delete(&self.layout.tail(key, piece.offset))
                    .await?; // ...so its pieces can go
            }
        }
        Ok(())
    }

    pub(super) async fn clear(&self, key: &str) -> Result<()> {
        for entry in self.api.list(&self.layout.tail_dir(key)).await? {
            self.api.delete(&entry.key).await?;
        }
        self.api.delete(&self.layout.state(key)).await
    }
}
