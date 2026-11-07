//! Listing a folder and searching.
//!
//! Pattern: Repository (read side) — builds backend-neutral `Query` values;
//! filtering, ordering and paging are done by the store, one extra row tells whether a next page exists.

use fs_store::{Op, Order, Query, StoredObject, TimeField};

use super::build::folder_key;
use super::consts::{CONTEXT, FOLDER_TYPE};
use super::handle::FileRepo;
use crate::error::Result;
use crate::model::{Cursor, Entry, File, FileFilter, Folder, Page, Sort, SortBy};

fn entry_of(o: &StoredObject) -> Result<Entry> {
    if o.envelope.object_type == FOLDER_TYPE {
        Ok(Entry::Folder(Folder::of(o)?))
    } else {
        Ok(Entry::File(File::of(o)?))
    }
}

/// Apply a sort order. Size and type use index fields that every record carries.
fn ordered(q: Query, sort: Sort) -> Result<Query> {
    let d = sort.descending;
    Ok(match sort.by {
        SortBy::Name => q.ordered(if d { Order::NameDesc } else { Order::NameAsc }),
        SortBy::Modified => q.ordered(if d {
            Order::UpdatedDesc
        } else {
            Order::UpdatedAsc
        }),
        SortBy::Size => q.order_by_index(if d { "size_desc" } else { "size_asc" }),
        SortBy::Type if d => {
            return Err(fs_interfaces::Unsupported::new(
                "file.sort",
                "sorting by type is ascending only",
            )
            .into());
        }
        SortBy::Type => q.order_by_index("type_asc"),
    })
}

impl FileRepo {
    pub(super) async fn do_list(
        &self,
        folder: Option<fs_store::ObjectId>,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("live_in", folder_key(folder));
        self.page_of(q, cursor, sort).await
    }

    pub(super) async fn do_find(
        &self,
        f: &FileFilter,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        let mut q = Query::new()
            .in_context(CONTEXT)
            .with_index("trashed", f.trashed.to_string());
        if let Some(name) = &f.name {
            q = q.name_contains(name.clone());
        }
        if let Some(prefix) = &f.media_type {
            q = q.index_op("media_type", Op::Prefix, prefix.to_ascii_lowercase());
        }
        let pad = |n: u64| format!("{n:020}");
        if let Some(min) = f.min_size {
            q = q.index_op("size_asc", Op::Ge, pad(min));
        }
        if let Some(max) = f.max_size {
            q = q.index_op("size_asc", Op::Le, pad(max));
        }
        for (field, op, at) in [
            (TimeField::Updated, Op::Ge, f.modified_after),
            (TimeField::Updated, Op::Lt, f.modified_before),
            (TimeField::Published, Op::Ge, f.created_after),
            (TimeField::Published, Op::Lt, f.created_before),
        ] {
            if let Some(t) = at {
                q = q.time(field, op, t);
            }
        }
        if let Some(folder) = f.folder {
            q = q.with_index("folder", folder.to_string());
        }
        self.page_of(q, cursor, sort).await
    }

    async fn page_of(&self, q: Query, cursor: Option<Cursor>, sort: Sort) -> Result<Page> {
        let start = cursor.map_or(0, |c| c.0);
        let mut q = ordered(q, sort)?.limit(self.page + 1);
        q.offset = start;
        let mut rows = self.reads.query(&q).await?;
        let next = (rows.len() > self.page).then(|| Cursor(start + self.page));
        rows.truncate(self.page);
        let items = rows.iter().map(entry_of).collect::<Result<Vec<_>>>()?;
        Ok(Page { items, next })
    }
}
