//! Listing a folder and searching.
//!
//! Pattern: Repository (read side) + Iterator-style paging — builds backend-neutral `Query` values;
//! filtering, ordering and paging are done by the store, one extra row tells whether a next page exists.

use fs_store::{Op, Order, Query, StoredObject};

use super::build::folder_key;
use super::consts::{CONTEXT, FOLDER_TYPE};
use super::handle::FileRepo;
use crate::error::Result;
use crate::model::{Cursor, Entry, File, FileFilter, Folder, Page};

fn entry_of(o: &StoredObject) -> Result<Entry> {
    if o.envelope.object_type == FOLDER_TYPE {
        Ok(Entry::Folder(Folder::of(o)?))
    } else {
        Ok(Entry::File(File::of(o)?))
    }
}

impl FileRepo {
    pub(super) async fn do_list(
        &self,
        folder: Option<fs_store::ObjectId>,
        cursor: Option<Cursor>,
    ) -> Result<Page> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("folder", folder_key(folder))
            .with_index("trashed", "false");
        self.page_of(q, cursor).await
    }

    pub(super) async fn do_find(&self, f: &FileFilter, cursor: Option<Cursor>) -> Result<Page> {
        let mut q = Query::new()
            .in_context(CONTEXT)
            .with_index("trashed", f.trashed.to_string());
        if let Some(name) = &f.name {
            q = q.name_contains(name.clone());
        }
        if let Some(prefix) = &f.media_type {
            q = q.index_op("media_type", Op::Prefix, prefix.to_ascii_lowercase());
        }
        if let Some(folder) = f.folder {
            q = q.with_index("folder", folder.to_string());
        }
        self.page_of(q, cursor).await
    }

    async fn page_of(&self, q: Query, cursor: Option<Cursor>) -> Result<Page> {
        let start = cursor.map_or(0, |c| c.0);
        let mut q = q.ordered(Order::NameAsc).limit(self.page + 1);
        q.offset = start;
        let mut rows = self.reads.query(&q).await?;
        let next = (rows.len() > self.page).then(|| Cursor(start + self.page));
        rows.truncate(self.page);
        let items = rows.iter().map(entry_of).collect::<Result<Vec<_>>>()?;
        Ok(Page { items, next })
    }
}
