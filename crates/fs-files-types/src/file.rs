//! `File`: the data type of a stored file (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — `DescribesType` is how the type enters the `TypeRegistry`; the
//! description is built with the Builder of `fs-types`. Standard: ActivityStreams 2.0 `Document` (W3C Recommendation);
//! the media type narrows it to `Image`, `Video` or `Audio`, as the core stores it (`DESIGN.md`). A file is a
//! document with bytes, a name and a media type; AS2 has no better term, and the
//! `Document` subtypes carry the same properties. Commands: everything the core has and everything
//! `requirements.md` of Files lists; `support.rs` says which of them the core answers today.

use fs_types::{
    BasicType, CommandDescription, CommandKind, DescribesType, FieldDescription, TypeDescription,
};

use crate::common::{
    PROGRAM, command, field, filter_fields, on_conflict, on_record, optional_record, record, text,
};
use crate::file_later::later;

/// The `File` type.
pub struct File;

fn fields() -> Vec<FieldDescription> {
    vec![
        text("name").required(),
        text("mediaType"),
        // Bytes as decimal text: `fs-types` has no quantity kind yet (named in `DESIGN.md`).
        field("size", BasicType::Text),
        text("checksum"),
        optional_record("folder", "Folder"),
        FieldDescription::new("state", BasicType::Choice)
            .choices(&["live", "trashed"])
            .label("files.state"),
        field("published", BasicType::DateTime),
        field("updated", BasicType::DateTime),
        field("sensitive", BasicType::Keywords),
    ]
}

fn find() -> CommandDescription {
    let mut c = command("file", "find", CommandKind::Find);
    for f in filter_fields() {
        c = c.input(f);
    }
    c.input(
        FieldDescription::new("state", BasicType::Choice)
            .choices(&["live", "trashed"])
            .label("files.state"),
    )
}

fn changes() -> Vec<CommandDescription> {
    vec![
        on_record("file", "rename", CommandKind::Change, "file", "File")
            .input(text("name").required())
            .input(on_conflict()),
        on_record("file", "move", CommandKind::Change, "file", "File")
            .input(optional_record("folder", "Folder"))
            .input(on_conflict()),
        on_record("file", "classify", CommandKind::Change, "file", "File")
            .input(field("sensitive", BasicType::Keywords)),
    ]
}

fn actions() -> Vec<CommandDescription> {
    vec![
        on_record("file", "copy", CommandKind::Other, "file", "File")
            .input(optional_record("folder", "Folder"))
            .input(on_conflict())
            .result(record("copy", "File")),
        on_record("file", "delete", CommandKind::Delete, "file", "File"),
        on_record("file", "restore", CommandKind::Other, "file", "File").input(on_conflict()),
        on_record("file", "restoreTo", CommandKind::Other, "file", "File")
            .input(optional_record("folder", "Folder"))
            .input(on_conflict()),
        on_record("file", "purge", CommandKind::Delete, "file", "File"),
        command("file", "emptyTrash", CommandKind::Other).result(field("purged", BasicType::Text)),
        on_record("file", "resolve", CommandKind::Other, "file", "File"),
        command("file", "upload", CommandKind::Create)
            .input(optional_record("folder", "Folder"))
            .input(text("name"))
            .input(text("mediaType"))
            .input(field("source", BasicType::Link).required())
            .input(on_conflict())
            .result(record("created", "File")),
        on_record("file", "download", CommandKind::Other, "file", "File")
            .input(field("target", BasicType::Link).required()),
        command("file", "uploadList", CommandKind::Other)
            .result(text("uploadId").multiple())
            .result(text("name").multiple())
            .result(field("received", BasicType::Text).multiple()),
        command("file", "uploadAbort", CommandKind::Other).input(text("uploadId").required()),
        command("file", "subscribe", CommandKind::Other),
    ]
}

impl DescribesType for File {
    fn describe() -> TypeDescription {
        let mut t = TypeDescription::new("File", PROGRAM, "AS2 Document", "file");
        for f in fields() {
            t = t.field(f);
        }
        t = t.command(find());
        for c in changes().into_iter().chain(actions()).chain(later()) {
            t = t.command(c);
        }
        t
    }
}
