//! Reading the lists: the pages of `find` and of a folder, up to a bound.
//!
//! Pattern: Iterator over pages (`coding.md` §2) — the contract hands out pages with a cursor; the frontend wants the
//! list, so the pages are read one after the other until the end or [`MAX_RECORDS`]. The strategy's rule: a list is
//! `find` with an empty search word and paging (user decision, 2026-11-10).

use fs_admin_frontend::{BackendError, Values};
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::{file_filter, search};
use fs_interfaces::rpc::write_uuid;

use crate::remote::{Clients, problem};
use crate::values::{Found, entry_record, id_of, millis, number, sort_of, text};

/// The most records one list reads. Beyond it the person narrows the search (the frontend has no paging yet).
pub(crate) const MAX_RECORDS: usize = 5000;

/// Put the typed `find` input into the filter of the contract. `media` fixes the media type prefix when given.
pub(crate) fn fill(
    mut f: file_filter::Builder<'_>,
    input: &Values,
    media: Option<&str>,
) -> Result<(), BackendError> {
    f.set_name(text(input, "name").unwrap_or_default());
    f.set_media_type(
        media
            .or_else(|| text(input, "mediaType"))
            .unwrap_or_default(),
    );
    if let Some(folder) = id_of(input, "folder")? {
        write_uuid(f.reborrow().init_folder(), Some(folder));
    }
    f.set_trashed(text(input, "state") == Some("trashed"));
    f.set_min_size(number(input, "minSize")?.unwrap_or(0));
    if let Some(max) = number(input, "maxSize")? {
        f.set_max_size(i64::try_from(max).unwrap_or(i64::MAX));
    }
    let times: [(&str, fn(&mut file_filter::Builder<'_>, i64)); 4] = [
        ("modifiedAfter", |f, t| {
            f.reborrow().init_modified_after().set_unix_millis(t)
        }),
        ("modifiedBefore", |f, t| {
            f.reborrow().init_modified_before().set_unix_millis(t)
        }),
        ("createdAfter", |f, t| {
            f.reborrow().init_created_after().set_unix_millis(t)
        }),
        ("createdBefore", |f, t| {
            f.reborrow().init_created_before().set_unix_millis(t)
        }),
    ];
    for (name, set) in times {
        if let Some(t) = millis(input, name)? {
            set(&mut f, t);
        }
    }
    Ok(())
}

/// Every entry `find` selects with `input`, in the order asked for.
pub(crate) async fn find_all(
    c: &Clients,
    input: &Values,
    media: Option<&str>,
) -> Result<Vec<Found>, BackendError> {
    let (by, descending) = sort_of(input);
    let mut found = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let mut call = c.search.find_request();
        fill(call.get().init_filter(), input, media)?;
        call.get().init_cursor().set_opaque(&cursor);
        {
            let mut s = call.get().init_sort();
            s.set_by(by);
            s.set_descending(descending);
        }
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        let page = answer
            .get()
            .and_then(search::find_results::Reader::get_page)
            .map_err(|e| problem(&e))?;
        for item in page.get_items().map_err(|e| problem(&e))? {
            found.push(entry_record(item).map_err(|e| problem(&e))?);
        }
        let next = page
            .get_next()
            .and_then(common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| problem(&e))?;
        if next.is_empty() || found.len() >= MAX_RECORDS {
            found.truncate(MAX_RECORDS);
            return Ok(found);
        }
        cursor = next.to_vec();
    }
}
