//! Ids and the small enums of a request.
//!
//! Pattern: Adapter — object ids (an unset one is "none" where the contract says so), `OnConflict`
//! and `Sort` between Rust and `files.capnp`. A value this server does not know is refused: a newer
//! client must not be answered as if it had asked for something else.

use fs_files_core::{OnConflict, Sort, SortBy};
use fs_interfaces::common_capnp::uuid;
use fs_interfaces::files_capnp;
use fs_interfaces::rpc::{invalid_request, read_uuid, write_uuid};
use fs_objects::ObjectId;

/// An id that may be unset (`None`: the top level, no folder).
pub(crate) fn optional(r: uuid::Reader<'_>) -> capnp::Result<Option<ObjectId>> {
    read_uuid(r)
        .map(|u| ObjectId::from_uuid(u).map_err(|e| invalid_request(e.to_string())))
        .transpose()
}

/// Write an id that may be unset.
pub(crate) fn write_optional(out: uuid::Builder<'_>, id: Option<ObjectId>) {
    write_uuid(out, id.map(|i| i.as_uuid()));
}

/// The rule for a taken name.
pub(crate) fn conflict(
    value: Result<files_capnp::OnConflict, capnp::NotInSchema>,
) -> capnp::Result<OnConflict> {
    match value {
        Ok(files_capnp::OnConflict::Refuse) => Ok(OnConflict::Refuse),
        Ok(files_capnp::OnConflict::KeepBoth) => Ok(OnConflict::KeepBoth),
        Err(e) => Err(invalid_request(format!("unknown onConflict {}", e.0))),
    }
}

/// The sort order; an unset one is by name, ascending.
pub(crate) fn sort(r: files_capnp::sort::Reader<'_>) -> capnp::Result<Sort> {
    let by = match r.get_by() {
        Ok(files_capnp::SortBy::Name) => SortBy::Name,
        Ok(files_capnp::SortBy::Modified) => SortBy::Modified,
        Ok(files_capnp::SortBy::Size) => SortBy::Size,
        Ok(files_capnp::SortBy::MediaType) => SortBy::Type,
        Err(e) => return Err(invalid_request(format!("unknown sort key {}", e.0))),
    };
    // `foldersFirst` (fs-interfaces v0.0.19, default true: a client that never writes it reads as true) is the client's
    // wish: a file manager wants the folders first, a plain listing may ask for the mixed order.
    let sort = Sort::new(by, r.get_descending());
    Ok(if r.get_folders_first() {
        sort.with_folders_first()
    } else {
        sort
    })
}
