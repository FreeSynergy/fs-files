//! The unfinished uploads, a page at a time.
//!
//! Pattern: Iterator (external, resumable) over the Repository — the page after a [`Cursor`] is asked
//! of the store with a limit and a place, never as the whole list that is cut afterwards. The order is
//! the store's (`published`, then the id), so the place is a count of sessions in that order. A
//! session whose file exists (only deleting the session failed) is skipped in a page and removed
//! once, when the list is read from its start, so a removal cannot shift a page in the middle.

use fs_objects::{ObjectId, Order, Query};

use super::handle::FileRepo;
use super::session::SessionRecord;
use crate::error::Result;
use crate::model::{Cursor, UploadInfo, UploadPage};

/// Sessions asked of the store at a time when the stale ones are looked for.
const SWEEP_BATCH: usize = 100;

impl FileRepo {
    fn sessions() -> Query {
        Query::new()
            .in_context(super::consts::CONTEXT)
            .with_index("kind", "upload")
            .ordered(Order::PublishedAsc)
    }

    /// Every upload that was started and not finished or aborted, oldest first, read page by page.
    pub(super) async fn do_pending_uploads(&self) -> Result<Vec<UploadInfo>> {
        let (mut out, mut cursor) = (Vec::new(), None);
        loop {
            let page = self.do_pending_page(cursor, SWEEP_BATCH).await?;
            out.extend(page.items);
            cursor = page.next;
            if cursor.is_none() {
                return Ok(out);
            }
        }
    }

    /// Up to `size` uploads after `cursor` (`None` = the start, which also removes stale sessions).
    pub(super) async fn do_pending_page(
        &self,
        cursor: Option<Cursor>,
        size: usize,
    ) -> Result<UploadPage> {
        let size = size.max(1);
        if cursor.is_none() {
            self.drop_stale_sessions().await?;
        }
        let mut at = cursor.map_or(0, Cursor::position);
        let mut items = Vec::new();
        while items.len() < size {
            let mut q = Self::sessions().limit(size - items.len());
            q.offset = at;
            let rows = self.reads.query(&q).await?;
            if rows.is_empty() {
                return Ok(UploadPage { items, next: None });
            }
            at += rows.len();
            for o in rows {
                if self.file_of_upload(o.envelope.id).await?.is_some() {
                    continue; // stale: the file exists, the first page removes the session
                }
                let rec: SessionRecord = serde_json::from_value(o.content.clone())?;
                let received = match self.backends.get(&rec.backend) {
                    Ok(b) => b.pending(&rec.key).await.unwrap_or(0),
                    Err(_) => 0, // the backend was removed: the upload cannot go on, but it is still listed
                };
                items.push(UploadInfo {
                    id: o.envelope.id.to_string(),
                    folder: rec.folder,
                    name: rec.name,
                    media_type: rec.media_type,
                    received,
                    started: o.envelope.published,
                });
            }
        }
        let mut more = Self::sessions().limit(1);
        more.offset = at;
        let next = (!self.reads.query(&more).await?.is_empty()).then(|| Cursor::at(at));
        Ok(UploadPage { items, next })
    }

    /// Remove the sessions whose file exists. Found first, removed after, so no page moves meanwhile.
    async fn drop_stale_sessions(&self) -> Result<()> {
        let (mut stale, mut at) = (Vec::<ObjectId>::new(), 0);
        loop {
            let mut q = Self::sessions().limit(SWEEP_BATCH);
            q.offset = at;
            let rows = self.reads.query(&q).await?;
            at += rows.len();
            for o in &rows {
                if self.file_of_upload(o.envelope.id).await?.is_some() {
                    stale.push(o.envelope.id);
                }
            }
            if rows.len() < SWEEP_BATCH {
                break;
            }
        }
        if !stale.is_empty() {
            let _w = self.lock.lock().await;
            for id in stale {
                let _ = self.objects.delete(id, &self.actor).await; // bytes untouched
            }
        }
        Ok(())
    }
}
