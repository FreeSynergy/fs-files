//! What the server says and what the workbench asks, turned into each other.
//!
//! Pattern: Adapter — pure functions between the records of `files.capnp` and the model of the workbench.

use std::fmt::Write;

use fs_admin_frontend::{BackendError, Values};
use fs_components_workbenches::file_manager::{
    FileEntry, FileError, FileProblem, ListRequest, NameRule, Scope, SortKey,
};
use fs_interfaces::files_capnp::{OnConflict, SortBy};
use fs_types::TypeProblem;
use uuid::Uuid;

use crate::remote::problem;
use crate::values::{Found, file_record};

/// The text of a time for the list: day and minute, in UTC.
pub(super) fn shown(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339).map_or_else(
        |_| rfc3339.to_owned(),
        |t| t.format("%Y-%m-%d %H:%M").to_string(),
    )
}

/// An entry of the workbench from a record of the server.
pub(super) fn entry_of(found: Found) -> FileEntry {
    let (record, folder) = match found {
        Found::File(r) => (r, false),
        Found::Folder(r) => (r, true),
    };
    let one = |k: &str| record.values.get(k).and_then(|v| v.first()).cloned();
    let key = bare(&record.id);
    let name = one("name").unwrap_or_default();
    let mut entry = if folder {
        FileEntry::folder(key, name)
    } else {
        FileEntry::file(
            key,
            name,
            one("size").and_then(|s| s.parse().ok()).unwrap_or(0),
        )
    };
    if !folder {
        entry.media_type = one("mediaType").unwrap_or_default();
        entry.trashed = one("state").as_deref() == Some("trashed");
        entry.sensitive = record.values.get("sensitive").cloned().unwrap_or_default();
    }
    entry.folder = one(if folder { "parent" } else { "folder" }).map(|f| bare(&f));
    entry.modified = one("updated").map(|t| shown(&t));
    entry
}

/// The bare id of `urn:uuid:<id>`.
pub(super) fn bare(id: &str) -> String {
    id.strip_prefix("urn:uuid:").unwrap_or(id).to_owned()
}

pub(super) fn id_of(key: &str) -> Result<Uuid, FileError> {
    Uuid::parse_str(key)
        .map_err(|_| FileError::new(FileProblem::NotFound, "this is not a known id"))
}

pub(super) fn folder_id(key: Option<&str>) -> Result<Option<Uuid>, FileError> {
    key.map(id_of).transpose()
}

pub(super) fn conflict(rule: NameRule) -> OnConflict {
    match rule {
        NameRule::KeepBoth => OnConflict::KeepBoth,
        _ => OnConflict::Refuse,
    }
}

/// What the server said, as the workbench understands it.
pub(super) fn error_of(error: BackendError) -> FileError {
    match error {
        BackendError::NotFound => FileError::new(FileProblem::NotFound, "it is not there any more"),
        BackendError::Rejected(issues) => {
            let kind = issues.first().and_then(|i| match &i.problem {
                fs_types::RecordProblem::Value(p) => Some(*p),
                _ => None,
            });
            match kind {
                Some(TypeProblem::NotInList) => {
                    FileError::new(FileProblem::NameConflict, "this name is taken")
                }
                Some(TypeProblem::Empty | TypeProblem::TooLong | TypeProblem::Character) => {
                    FileError::new(FileProblem::InvalidName, "this name cannot be used")
                }
                _ => FileError::failed("the server did not accept this"),
            }
        }
        BackendError::Unavailable(text) => match text.strip_prefix("unsupported:") {
            Some(name) => FileError::unsupported(format!("not possible yet: {name}")),
            None => FileError::failed(text),
        },
        BackendError::Handshake(text) => FileError::failed(text),
    }
}

/// A time as the server wants it: RFC 3339, or a bare day (midnight UTC).
pub(super) fn instant(text: &str) -> String {
    let text = text.trim();
    if chrono::DateTime::parse_from_rfc3339(text).is_ok() {
        text.to_owned()
    } else {
        format!("{text}T00:00:00Z")
    }
}

/// The query of `find` for what the person asked.
pub(super) fn query_of(request: &ListRequest) -> Values {
    let mut q = Values::new();
    let mut put = |k: &str, v: String| {
        if !v.is_empty() {
            q.insert(k.to_owned(), vec![v]);
        }
    };
    put("name", request.term.clone());
    match &request.scope {
        Scope::Folder(Some(f)) => put("folder", f.clone()),
        Scope::Trash => put("state", "trashed".into()),
        _ => {}
    }
    let f = &request.filter;
    put(
        "minSize",
        f.min_size.map(|n| n.to_string()).unwrap_or_default(),
    );
    put(
        "maxSize",
        f.max_size.map(|n| n.to_string()).unwrap_or_default(),
    );
    put(
        "modifiedAfter",
        f.modified_from.as_deref().map(instant).unwrap_or_default(),
    );
    put(
        "modifiedBefore",
        f.modified_before
            .as_deref()
            .map(instant)
            .unwrap_or_default(),
    );
    q
}

pub(super) fn sort_by(key: SortKey) -> SortBy {
    match key {
        SortKey::Modified => SortBy::Modified,
        SortKey::Size => SortBy::Size,
        SortKey::Type => SortBy::MediaType,
        _ => SortBy::Name,
    }
}

/// The `file:` link of a path, as the transfer commands read it.
pub(super) fn link_of(path: &str) -> String {
    link_text(path, cfg!(windows))
}

/// The link of `path`; with `windows` the separators become `/` and the drive letter follows the first slash
/// (`C:\Users\me` is `file:///C%3A/Users/me`), the form `link::current().path_of` reads back.
fn link_text(path: &str, windows: bool) -> String {
    let slashed;
    let mut path = path;
    if windows {
        slashed = path.replace('\\', "/");
        path = &slashed;
    }
    let mut out = String::from("file://");
    if windows && !path.starts_with('/') {
        out.push('/');
    }
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

pub(super) fn new_name(
    file: fs_interfaces::files_capnp::file::Reader<'_>,
) -> Result<String, BackendError> {
    let record = file_record(file).map_err(|e| problem(&e))?;
    Ok(record
        .values
        .get("name")
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default())
}

pub(super) fn text_name(entry: &FileEntry) -> String {
    entry.name.clone()
}

#[cfg(test)]
mod link_tests {
    use super::link_text;

    #[test]
    fn a_unix_path_keeps_its_slashes_and_encodes_the_rest() {
        assert_eq!(link_text("/tmp/a b.txt", false), "file:///tmp/a%20b.txt");
    }

    #[test]
    fn a_windows_path_has_slashes_and_the_drive_after_the_first_one() {
        assert_eq!(
            link_text(r"C:\Users\me\a b.txt", true),
            "file:///C%3A/Users/me/a%20b.txt"
        );
    }
}
