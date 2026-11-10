//! `Picture`: the data type of the gallery facet for an image (ADR 0066).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — `DescribesType`. Standard: ActivityStreams 2.0 `Image` (the
//! gallery requirements name `Image` / `Video` with a reference to the file record). A picture refers to its `File`
//! (`file`), the bytes stay in `fs-files`; the core has no metadata object for it yet, so what is derived from the file
//! (name, media type, size) is answered today and the rest (`takenAt`, `place`, `keywords` …) is empty. Commands: the
//! gallery requirements in full; `picture_later.rs` holds those the core does not have.

use fs_types::{
    BasicType, CommandDescription, CommandKind, DescribesType, FieldDescription, TypeDescription,
};

use crate::common::{PROGRAM, command, field, filter_fields, on_record, text};
use crate::picture_later::later;

/// The `Picture` type.
pub struct Picture;

fn fields() -> Vec<FieldDescription> {
    vec![
        crate::common::record("file", "File"),
        text("name").required(),
        text("mediaType"),
        field("size", BasicType::Text),
        field("takenAt", BasicType::DateTime),
        field("description", BasicType::MultilineText),
        field("place", BasicType::Place),
        field("keywords", BasicType::Keywords),
        field("sensitive", BasicType::Keywords),
    ]
}

fn on_picture(name: &str, kind: CommandKind) -> CommandDescription {
    on_record("picture", name, kind, "picture", "Picture")
}

fn supported() -> Vec<CommandDescription> {
    let mut find = command("picture", "find", CommandKind::Find);
    // The media type is fixed to images by the backend; the shared filter fields stay for the rest.
    for f in filter_fields()
        .into_iter()
        .filter(|f| f.name != "mediaType")
    {
        find = find.input(f);
    }
    vec![
        find,
        on_picture("download", CommandKind::Other)
            .input(field("target", BasicType::Link).required()),
        on_picture("sensitiveMark", CommandKind::Change)
            .input(field("sensitive", BasicType::Keywords)),
    ]
}

impl DescribesType for Picture {
    fn describe() -> TypeDescription {
        let mut t = TypeDescription::new("Picture", PROGRAM, "AS2 Image", "picture");
        for f in fields() {
            t = t.field(f);
        }
        for c in supported().into_iter().chain(later()) {
            t = t.command(c);
        }
        t
    }
}
