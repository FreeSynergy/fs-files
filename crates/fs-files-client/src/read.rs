//! What the server sends, as the values of the core.
//!
//! Pattern: Adapter — `File`, `Folder`, `Entry`, `UploadInfo` and the two reports of `files.capnp` to
//! the types of `fs-files-core`. A reply that cannot be read is a backend error saying so: the request
//! is not to blame.

use fs_files_core::{Entry, Error, File, Folder, RescanReport, Result, TrashReport, UploadInfo};
use fs_interfaces::common_capnp::uuid;
use fs_interfaces::files_capnp::{entry, file, folder, rescan_report, trash_report, upload_info};
use fs_interfaces::rpc::{from_millis, read_envelope, read_object_id, read_uuid};
use fs_store::ObjectId;

/// A reply that does not read.
pub(crate) fn unreadable(e: impl std::fmt::Display) -> Error {
    Error::Backend(format!("files server: it sent something unreadable: {e}"))
}

fn wire<T>(r: capnp::Result<T>) -> Result<T> {
    r.map_err(unreadable)
}

fn text(t: capnp::Result<capnp::text::Reader<'_>>) -> Result<String> {
    wire(t.and_then(|t| Ok(t.to_string()?)))
}

/// An id that may be unset.
pub(crate) fn optional(r: uuid::Reader<'_>) -> Result<Option<ObjectId>> {
    read_uuid(r)
        .map(|u| ObjectId::from_uuid(u).map_err(unreadable))
        .transpose()
}

/// A file: the envelope as sent; name, IRI and marks are the envelope's.
pub(crate) fn file(r: file::Reader<'_>) -> Result<File> {
    let envelope = wire(r.get_envelope().and_then(read_envelope))?;
    Ok(File {
        id: envelope.id,
        iri: envelope
            .iri
            .clone()
            .ok_or_else(|| unreadable("a file without an iri"))?,
        name: envelope.name.clone(),
        media_type: text(r.get_media_type())?,
        size: r.get_size(),
        checksum: text(r.get_checksum())?,
        folder: optional(wire(r.get_folder())?)?,
        trashed: r.get_trashed(),
        sensitive: envelope.sensitive.clone(),
        envelope,
    })
}

/// A folder.
pub(crate) fn folder(r: folder::Reader<'_>) -> Result<Folder> {
    let envelope = wire(r.get_envelope().and_then(read_envelope))?;
    Ok(Folder {
        id: envelope.id,
        iri: envelope
            .iri
            .clone()
            .ok_or_else(|| unreadable("a folder without an iri"))?,
        name: envelope.name.clone(),
        parent: optional(wire(r.get_parent())?)?,
        envelope,
    })
}

/// An entry of a page.
pub(crate) fn entry(r: entry::Reader<'_>) -> Result<Entry> {
    match r.which().map_err(unreadable)? {
        entry::Which::File(f) => file(wire(f)?).map(Entry::File),
        entry::Which::Folder(f) => folder(wire(f)?).map(Entry::Folder),
    }
}

/// An unfinished upload.
pub(crate) fn upload(r: upload_info::Reader<'_>) -> Result<UploadInfo> {
    Ok(UploadInfo {
        id: text(r.get_id())?,
        folder: optional(wire(r.get_folder())?)?,
        name: text(r.get_name())?,
        media_type: text(r.get_media_type())?,
        received: r.get_received(),
        started: wire(
            r.get_started()
                .and_then(|t| from_millis(t.get_unix_millis())),
        )?,
    })
}

/// The report of a rescan.
pub(crate) fn rescan(r: rescan_report::Reader<'_>) -> Result<RescanReport> {
    let skipped = wire(r.get_skipped())?;
    Ok(RescanReport {
        added: wire(r.get_added())?
            .iter()
            .map(entry)
            .collect::<Result<_>>()?,
        updated: wire(r.get_updated())?
            .iter()
            .map(file)
            .collect::<Result<_>>()?,
        missing: wire(r.get_missing())?
            .iter()
            .map(file)
            .collect::<Result<_>>()?,
        skipped: skipped.iter().map(text).collect::<Result<_>>()?,
    })
}

/// The report of emptying the trash. `failedMissing` (ids the server could not list) has no place in
/// the core's type; `DESIGN.md` names it.
pub(crate) fn trash(r: trash_report::Reader<'_>) -> Result<TrashReport> {
    Ok(TrashReport {
        purged: r.get_purged(),
        failed: wire(r.get_failed())?
            .iter()
            .map(|id| wire(read_object_id(id)))
            .collect::<Result<_>>()?,
    })
}
