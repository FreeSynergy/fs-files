//! A search that follows the cursor to the end, and a small tree to search in.

#![allow(dead_code)] // each test file uses a different subset

use super::{Caps, mkdir, put_file, read_entry, text_file};
use fs_interfaces::files_capnp::{OnConflict, SortBy, file_filter};

pub async fn find(
    c: &Caps,
    filter: impl Fn(file_filter::Builder<'_>),
    sort: Option<(SortBy, bool)>,
) -> capnp::Result<Vec<String>> {
    find_with(c, filter, sort, true).await
}

/// [`find`] with the wish of the client about the folders: `false` asks for folders and files mixed.
pub async fn find_with(
    c: &Caps,
    filter: impl Fn(file_filter::Builder<'_>),
    sort: Option<(SortBy, bool)>,
    folders_first: bool,
) -> capnp::Result<Vec<String>> {
    let (mut out, mut cursor) = (vec![], vec![]);
    loop {
        let mut call = c.search.find_request();
        filter(call.get().init_filter());
        call.get().init_cursor().set_opaque(&cursor);
        if let Some((by, descending)) = sort {
            let mut order = call.get().init_sort();
            order.set_by(by);
            order.set_descending(descending);
            order.set_folders_first(folders_first);
        }
        let answer = call.send().promise.await?;
        let page = answer.get()?.get_page()?;
        out.extend(page.get_items()?.iter().map(|e| read_entry(e).0));
        cursor = page.get_next()?.get_opaque()?.to_vec();
        if cursor.is_empty() {
            return Ok(out);
        }
    }
}

/// A folder `Photos` with two files, and a file of 5 bytes beside it.
pub async fn fill(c: &Caps) -> uuid::Uuid {
    let dir = mkdir(c, None, "Photos").await.unwrap();
    text_file(c, None, "notes.txt", "12345").await;
    text_file(c, Some(dir), "trip.txt", "1").await;
    let png = vec![7; 300];
    let put = put_file(
        c,
        Some(dir),
        "cat.png",
        "image/png",
        &png,
        OnConflict::Refuse,
    );
    put.await.unwrap();
    dir
}
