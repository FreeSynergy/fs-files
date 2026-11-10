//! `Folder`: the data type of a folder (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — `DescribesType`. Standard: ActivityStreams 2.0 `Collection`: a
//! folder is a collection of files and folders, as the core stores it (`DESIGN.md`). `OrderedCollection` is not used:
//! the order is a view parameter (the `sortBy` of `find`), not a property of the folder.

use fs_types::{
    BasicType, CommandDescription, CommandKind, DescribesType, FieldDescription, TypeDescription,
};

use crate::common::{PROGRAM, command, field, on_conflict, on_record, optional_record, text};

/// The `Folder` type.
pub struct Folder;

fn fields() -> Vec<FieldDescription> {
    vec![
        text("name").required(),
        optional_record("parent", "Folder"),
        field("published", BasicType::DateTime),
        field("updated", BasicType::DateTime),
    ]
}

fn commands() -> Vec<CommandDescription> {
    vec![
        command("folder", "find", CommandKind::Find).input(text("name")),
        command("folder", "create", CommandKind::Create)
            .input(parent())
            .input(text("name").required())
            .result(on_record_ref()),
        on_record("folder", "rename", CommandKind::Change, "folder", "Folder")
            .input(text("name").required())
            .input(on_conflict()),
        on_record("folder", "move", CommandKind::Change, "folder", "Folder")
            .input(parent())
            .input(on_conflict()),
        on_record("folder", "delete", CommandKind::Delete, "folder", "Folder"),
        on_record("folder", "resolve", CommandKind::Other, "folder", "Folder"),
        on_record("folder", "rescan", CommandKind::Other, "folder", "Folder")
            .result(text("added").multiple())
            .result(text("updated").multiple())
            .result(text("missing").multiple())
            .result(text("skipped").multiple()),
        on_record("folder", "archive", CommandKind::Other, "folder", "Folder")
            .input(field("target", BasicType::Link).required()),
    ]
}

/// The folder to put it in, or none for the top. No `target`: a reference to the own type would make the frontend take the
/// selected record for it (the rule for commands on a record) and the top level could not be chosen.
fn parent() -> FieldDescription {
    FieldDescription::new("parent", BasicType::Reference).label("files.parent")
}

fn on_record_ref() -> FieldDescription {
    crate::common::record("created", "Folder")
}

impl DescribesType for Folder {
    fn describe() -> TypeDescription {
        let mut t = TypeDescription::new("Folder", PROGRAM, "AS2 Collection", "folder");
        for f in fields() {
            t = t.field(f);
        }
        for c in commands() {
            t = t.command(c);
        }
        t
    }
}
