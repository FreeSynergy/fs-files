//! The types of the provider interface `BlobBackend` on the wire.
//!
//! Pattern: Adapter — `Spot`, `BlobInfo` and `Scanned` between Rust and `files.capnp`. A key or path
//! beyond any legitimate length is refused at the door; a listing that would not fit a message is
//! refused whole (the contract has no paging for `scan`).

use fs_files_core::{BlobInfo, ScanKind, Scanned, Spot};
use fs_interfaces::files_capnp::{blob_info, scanned, spot};
use fs_interfaces::rpc::{check_answer, invalid_request};

use crate::wire::count;
use crate::wire::text::{MAX_TERM, limited};

/// A key or a path.
pub(crate) fn key(text: capnp::text::Reader<'_>) -> capnp::Result<&str> {
    limited(text, "key", MAX_TERM)
}

/// Read a spot: a path, an unfinished upload or a trash entry.
pub(crate) fn read_spot(r: spot::Reader<'_>) -> capnp::Result<Spot> {
    Ok(match r.which() {
        Ok(spot::Which::Path(t)) => Spot::Path(key(t?)?.to_string()),
        Ok(spot::Which::Pending(t)) => Spot::Pending(key(t?)?.to_string()),
        Ok(spot::Which::Trash(t)) => Spot::Trash(key(t?)?.to_string()),
        Err(e) => return Err(invalid_request(format!("unknown kind of spot {}", e.0))),
    })
}

/// Write size, checksum and modification time of a blob.
pub(crate) fn write_info(mut out: blob_info::Builder<'_>, info: &BlobInfo) {
    out.set_size(info.size);
    out.set_checksum(info.checksum.as_str());
    out.set_modified(info.modified);
}

/// Write the entries of one folder, sorted as the backend gave them.
pub(crate) fn write_scanned(
    mut out: capnp::struct_list::Builder<'_, scanned::Owned>,
    items: &[Scanned],
) {
    for (i, s) in (0u32..).zip(items) {
        let mut item = out.reborrow().get(i);
        item.set_name(s.name.as_str());
        item.set_size(s.size);
        item.set_modified(s.modified);
        match &s.kind {
            ScanKind::File => item.set_kind(fs_interfaces::files_capnp::ScanKind::File),
            ScanKind::Dir => item.set_kind(fs_interfaces::files_capnp::ScanKind::Dir),
            ScanKind::Skipped(why) => {
                item.set_kind(fs_interfaces::files_capnp::ScanKind::Skipped);
                item.set_skip_reason(why.as_str());
            }
        }
    }
}

/// What a listing weighs, and the length for its list; refused if it cannot be sent.
pub(crate) fn scanned_len(items: &[Scanned]) -> capnp::Result<u32> {
    let weight = items.iter().map(|s| {
        let why = match &s.kind {
            ScanKind::Skipped(why) => why.len(),
            _ => 0,
        };
        96 + s.name.len() + why
    });
    check_answer(weight.sum())?;
    count(items.len())
}
