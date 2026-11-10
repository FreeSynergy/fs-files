//! Records and typed values: the contract's readers turned into the frontend's `Record`, and the frontend's texts
//! turned into ids, numbers and times for the contract.
//!
//! Pattern: Adapter — pure functions between `files.capnp` readers and `fs-admin-frontend` values.

use fs_admin_frontend::{BackendError, Record, Values};
use fs_interfaces::files_capnp::{OnConflict, SortBy, backend_info, entry, file, folder};
use fs_interfaces::rpc::{read_envelope, read_uuid};
use uuid::Uuid;

/// One value for a field.
fn one(values: &mut Values, name: &str, value: impl Into<String>) {
    values.insert(name.to_owned(), vec![value.into()]);
}

/// A file as the record of the type `File`.
pub(crate) fn file_record(f: file::Reader<'_>) -> capnp::Result<Record> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut v = Values::new();
    one(&mut v, "name", envelope.name);
    one(&mut v, "mediaType", f.get_media_type()?.to_string()?);
    one(&mut v, "size", f.get_size().to_string());
    one(&mut v, "checksum", f.get_checksum()?.to_string()?);
    if let Some(folder) = read_uuid(f.get_folder()?) {
        one(&mut v, "folder", folder.to_string());
    }
    one(
        &mut v,
        "state",
        if f.get_trashed() { "trashed" } else { "live" },
    );
    one(&mut v, "published", envelope.published.to_rfc3339());
    one(&mut v, "updated", envelope.updated.to_rfc3339());
    if !envelope.sensitive.is_empty() {
        v.insert("sensitive".to_owned(), envelope.sensitive);
    }
    Ok(Record {
        id: envelope.id.to_string(),
        values: v,
    })
}

/// A folder as the record of the type `Folder`.
pub(crate) fn folder_record(f: folder::Reader<'_>) -> capnp::Result<Record> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut v = Values::new();
    one(&mut v, "name", envelope.name);
    if let Some(parent) = read_uuid(f.get_parent()?) {
        one(&mut v, "parent", parent.to_string());
    }
    one(&mut v, "published", envelope.published.to_rfc3339());
    one(&mut v, "updated", envelope.updated.to_rfc3339());
    Ok(Record {
        id: envelope.id.to_string(),
        values: v,
    })
}

/// A backend as the record of the type `StorageBackend`; its id is its name.
pub(crate) fn storage_record(b: backend_info::Reader<'_>) -> capnp::Result<Record> {
    let id = b.get_id()?.to_string()?;
    let mut v = Values::new();
    one(&mut v, "name", id.clone());
    one(&mut v, "kind", b.get_kind()?.to_string()?);
    one(
        &mut v,
        "state",
        if b.get_active() { "active" } else { "idle" },
    );
    Ok(Record { id, values: v })
}

/// An entry of a page: a file or a folder.
pub(crate) enum Found {
    File(Record),
    Folder(Record),
}

pub(crate) fn entry_record(e: entry::Reader<'_>) -> capnp::Result<Found> {
    Ok(match e.which()? {
        entry::Which::File(f) => Found::File(file_record(f?)?),
        entry::Which::Folder(f) => Found::Folder(folder_record(f?)?),
    })
}

pub(crate) fn bad(what: &str) -> BackendError {
    BackendError::Unavailable(format!("bad input: {what}"))
}

/// The first value of a field, trimmed; `None` if absent or empty.
pub(crate) fn text<'a>(input: &'a Values, name: &str) -> Option<&'a str> {
    input
        .get(name)
        .and_then(|v| v.first())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// An id from the first value of a field.
pub(crate) fn id_of(input: &Values, name: &str) -> Result<Option<Uuid>, BackendError> {
    text(input, name)
        .map(|s| s.parse::<Uuid>().map_err(|_| BackendError::NotFound))
        .transpose()
}

/// A required id.
pub(crate) fn need_id(input: &Values, name: &str) -> Result<Uuid, BackendError> {
    id_of(input, name)?.ok_or(BackendError::NotFound)
}

pub(crate) fn on_conflict(input: &Values) -> OnConflict {
    match text(input, "onConflict") {
        Some("keep-both") => OnConflict::KeepBoth,
        _ => OnConflict::Refuse,
    }
}

pub(crate) fn sort_of(input: &Values) -> (SortBy, bool) {
    let by = match text(input, "sortBy") {
        Some("modified") => SortBy::Modified,
        Some("size") => SortBy::Size,
        Some("media-type") => SortBy::MediaType,
        _ => SortBy::Name,
    };
    (by, text(input, "order") == Some("descending"))
}

/// Milliseconds since the epoch of an RFC 3339 time.
pub(crate) fn millis(input: &Values, name: &str) -> Result<Option<i64>, BackendError> {
    text(input, name)
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|t| t.timestamp_millis())
                .map_err(|_| bad(name))
        })
        .transpose()
}

pub(crate) fn number(input: &Values, name: &str) -> Result<Option<u64>, BackendError> {
    text(input, name)
        .map(|s| s.parse::<u64>().map_err(|_| bad(name)))
        .transpose()
}
