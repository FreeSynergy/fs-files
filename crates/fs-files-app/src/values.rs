//! Records and typed values: the contract's readers turned into the frontend's `Record`, and the frontend's texts
//! turned into ids, numbers and times for the contract.
//!
//! Pattern: Adapter — pure functions between `files.capnp` readers and `fs-admin-frontend` values.

use fs_admin_frontend::{BackendError, Record, Values};
use fs_interfaces::files_capnp::{OnConflict, SortBy, backend_info, entry, file, folder};
use fs_interfaces::rpc::{read_envelope, read_uuid};
use fs_types::{RecordIssue, RecordProblem, TypeProblem};
use uuid::Uuid;

/// The id of a record as the frontend wants it: an absolute IRI (`urn:uuid:<id>`); a bare uuid is no reference.
pub(crate) fn urn(id: Uuid) -> String {
    id.urn().to_string()
}

/// A bare uuid as `urn:uuid:…`; anything else as it is.
pub(crate) fn as_iri(id: &str) -> String {
    Uuid::parse_str(id).map_or_else(|_| id.to_owned(), urn)
}

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
        one(&mut v, "folder", urn(folder));
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
        id: urn(envelope.id.as_uuid()),
        values: v,
    })
}

/// A folder as the record of the type `Folder`.
pub(crate) fn folder_record(f: folder::Reader<'_>) -> capnp::Result<Record> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut v = Values::new();
    one(&mut v, "name", envelope.name);
    if let Some(parent) = read_uuid(f.get_parent()?) {
        one(&mut v, "parent", urn(parent));
    }
    one(&mut v, "published", envelope.published.to_rfc3339());
    one(&mut v, "updated", envelope.updated.to_rfc3339());
    Ok(Record {
        id: urn(envelope.id.as_uuid()),
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

/// A finding on `field`: what was typed is not usable (a number, a time or an id that does not parse).
pub(crate) fn bad(field: &str) -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: field.to_owned(),
        index: 0,
        problem: RecordProblem::Value(TypeProblem::Character),
    }])
}

/// The first value of a field, trimmed; `None` if absent or empty.
pub(crate) fn text<'a>(input: &'a Values, name: &str) -> Option<&'a str> {
    input
        .get(name)
        .and_then(|v| v.first())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// The input of a command with the record it works on: the frontend hands the id of the record next to the input, the
/// calls below read it under the name of the field (`file`, `folder`, ...). A value the input already has stays.
pub(crate) fn with_record(input: &Values, key: &str, record: Option<&str>) -> Values {
    let mut input = input.clone();
    if let Some(record) = record {
        input
            .entry(key.to_owned())
            .or_insert_with(|| vec![record.to_owned()]);
    }
    input
}

/// An id from the first value of a field.
pub(crate) fn id_of(input: &Values, name: &str) -> Result<Option<Uuid>, BackendError> {
    text(input, name)
        .map(|s| Uuid::parse_str(s).map_err(|_| bad(name)))
        .transpose()
}

/// A required id.
pub(crate) fn need_id(input: &Values, name: &str) -> Result<Uuid, BackendError> {
    id_of(input, name)?.ok_or_else(|| bad(name))
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
