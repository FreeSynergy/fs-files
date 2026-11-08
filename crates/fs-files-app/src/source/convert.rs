//! Conversions between the contract's readers and the workbench's entries, pages and cursors.
//!
//! Pattern: Adapter — pure functions from `files.capnp` readers and rules to workbench types.

use std::sync::Arc;

use fs_components_workbenches::file_manager::{
    EntryPage, FileEntry, FileError, NameRule, PageMark, SortKey,
};
use fs_interfaces::files_capnp::{OnConflict, SortBy, entry, file, folder};
use fs_interfaces::rpc::{read_envelope, read_uuid};
use uuid::Uuid;

pub(super) fn id(key: &str) -> Result<Uuid, FileError> {
    key.parse().map_err(|_| {
        FileError::new(
            fs_components_workbenches::file_manager::FileProblem::NotFound,
            format!("\"{key}\" names nothing."),
        )
    })
}

pub(super) fn optional(key: Option<&str>) -> Result<Option<Uuid>, FileError> {
    key.map(id).transpose()
}

pub(super) fn on_conflict(rule: NameRule) -> OnConflict {
    match rule {
        NameRule::KeepBoth => OnConflict::KeepBoth,
        _ => OnConflict::Refuse,
    }
}

pub(super) fn sort_by(key: SortKey) -> SortBy {
    match key {
        SortKey::Modified => SortBy::Modified,
        SortKey::Size => SortBy::Size,
        SortKey::Type => SortBy::MediaType,
        _ => SortBy::Name,
    }
}

pub(super) fn file_of(f: file::Reader<'_>) -> capnp::Result<FileEntry> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut e = FileEntry::file(envelope.id.to_string(), envelope.name, f.get_size());
    e.media_type = f.get_media_type()?.to_string()?;
    e.modified = Some(envelope.updated.to_rfc3339());
    e.folder = read_uuid(f.get_folder()?).map(|u| u.to_string());
    e.trashed = f.get_trashed();
    Ok(e)
}

pub(super) fn folder_of(f: folder::Reader<'_>) -> capnp::Result<FileEntry> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut e = FileEntry::folder(envelope.id.to_string(), envelope.name);
    e.modified = Some(envelope.updated.to_rfc3339());
    e.folder = read_uuid(f.get_parent()?).map(|u| u.to_string());
    Ok(e)
}

pub(super) fn entry_of(e: entry::Reader<'_>) -> capnp::Result<FileEntry> {
    match e.which()? {
        entry::Which::File(f) => file_of(f?),
        entry::Which::Folder(f) => folder_of(f?),
    }
}

/// The cursor a page mark stands for; the first page has none.
pub(super) fn cursor_of(after: Option<&PageMark>) -> Vec<u8> {
    after
        .and_then(|m| m.0.downcast_ref::<Vec<u8>>())
        .cloned()
        .unwrap_or_default()
}

pub(super) fn page_of(
    items: capnp::struct_list::Reader<'_, entry::Owned>,
    next: &[u8],
) -> capnp::Result<EntryPage> {
    let mut entries = Vec::new();
    for item in items {
        entries.push(entry_of(item)?);
    }
    let mark = (!next.is_empty()).then(|| PageMark(Arc::new(next.to_vec())));
    Ok(EntryPage::new(entries, mark))
}
