//! The parts of a request that are more than an id: the rule, the order, the filter.
//!
//! Pattern: Adapter — `OnConflict`, `Sort` and `FileFilter` of the core as `files.capnp` writes them;
//! the inverse of the server's `wire/ids.rs` and `wire/filter.rs`.

use fs_files_core::{FileFilter, OnConflict, Sort, SortBy};
use fs_interfaces::files_capnp::{self, file_filter, sort};
use fs_interfaces::rpc::to_millis;

use fs_interfaces::rpc::write_uuid;

/// The rule for a taken name.
pub(crate) fn conflict(on: OnConflict) -> files_capnp::OnConflict {
    match on {
        OnConflict::Refuse => files_capnp::OnConflict::Refuse,
        OnConflict::KeepBoth => files_capnp::OnConflict::KeepBoth,
    }
}

/// The sort order.
pub(crate) fn order(mut out: sort::Builder<'_>, by: Sort) {
    out.set_by(match by.by {
        SortBy::Name => files_capnp::SortBy::Name,
        SortBy::Modified => files_capnp::SortBy::Modified,
        SortBy::Size => files_capnp::SortBy::Size,
        SortBy::Type => files_capnp::SortBy::MediaType,
    });
    out.set_descending(by.descending);
}

/// The filter; an unset condition stays unset (`maxSize` -1 is no limit).
pub(crate) fn filter(mut out: file_filter::Builder<'_>, f: &FileFilter) {
    out.set_name(f.name.as_deref().unwrap_or_default());
    out.set_media_type(f.media_type.as_deref().unwrap_or_default());
    top(out.reborrow().init_folder(), f.folder);
    out.set_trashed(f.trashed);
    out.set_min_size(f.min_size.unwrap_or(0));
    out.set_max_size(f.max_size.and_then(|m| i64::try_from(m).ok()).unwrap_or(-1));
    if let Some(t) = f.modified_after {
        out.reborrow()
            .init_modified_after()
            .set_unix_millis(to_millis(t));
    }
    if let Some(t) = f.modified_before {
        out.reborrow()
            .init_modified_before()
            .set_unix_millis(to_millis(t));
    }
    if let Some(t) = f.created_after {
        out.reborrow()
            .init_created_after()
            .set_unix_millis(to_millis(t));
    }
    if let Some(t) = f.created_before {
        out.reborrow()
            .init_created_before()
            .set_unix_millis(to_millis(t));
    }
}

/// An id that may be unset, written where the contract says "unset = the top level".
pub(crate) fn top(
    out: fs_interfaces::common_capnp::uuid::Builder<'_>,
    id: Option<fs_store::ObjectId>,
) {
    write_uuid(out, id.map(|i| i.as_uuid()));
}
