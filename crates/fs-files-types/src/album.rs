//! `Album`: the data type of the gallery facet for a set of pictures (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — `DescribesType`. Standard: `ActivityStreams` 2.0 `Collection`
//! (`requirements.md` of the gallery names `Collection` / `OrderedCollection`). `Collection` is chosen because the order
//! of an album is a view parameter (`album.sort`), as for a folder, not part of the data; the items are references to
//! pictures, never copies. The core has no album object yet: every command is unsupported (`support.rs`).

use fs_types::{BasicType, CommandKind, DescribesType, FieldDescription, TypeDescription};

use crate::common::{PROGRAM, command, field, on_record, optional_record, record, text};

/// The `Album` type.
pub struct Album;

fn on_album(name: &str, kind: CommandKind) -> fs_types::CommandDescription {
    on_record("album", name, kind, "album", "Album")
}

impl DescribesType for Album {
    fn describe() -> TypeDescription {
        TypeDescription::new("Album", PROGRAM, "AS2 Collection", "album")
            .field(text("name").required())
            .field(field("description", BasicType::MultilineText))
            .field(optional_record("cover", "Picture"))
            .field(
                FieldDescription::new("items", BasicType::Reference)
                    .target("Picture")
                    .multiple()
                    .label("files.items"),
            )
            .field(text("query"))
            .field(field("published", BasicType::DateTime))
            .field(field("updated", BasicType::DateTime))
            .command(command("album", "find", CommandKind::Find).input(text("name")))
            .command(
                command("album", "create", CommandKind::Create)
                    .input(text("name").required())
                    .input(field("description", BasicType::MultilineText))
                    // No `target`: nesting under an album must not make the frontend take the selected album for it.
                    .input(
                        FieldDescription::new("parent", BasicType::Reference).label("files.parent"),
                    ),
            )
            .command(
                on_album("edit", CommandKind::Change)
                    .input(text("name").required())
                    .input(field("description", BasicType::MultilineText)),
            )
            .command(on_album("delete", CommandKind::Delete))
            .command(on_album("add", CommandKind::Other).input(record("picture", "Picture")))
            .command(on_album("remove", CommandKind::Other).input(record("picture", "Picture")))
            .command(
                on_album("nest", CommandKind::Change).input(
                    FieldDescription::new("parent", BasicType::Reference).label("files.parent"),
                ),
            )
            .command(on_album("query", CommandKind::Change).input(text("query")))
            .command(command("album", "suggest", CommandKind::Other))
            .command(
                on_album("sort", CommandKind::Change)
                    .input(optional_record("cover", "Picture"))
                    .input(
                        FieldDescription::new("sortBy", BasicType::Choice)
                            .choices(&["taken", "name", "manual"])
                            .label("files.sort-by"),
                    ),
            )
    }
}
