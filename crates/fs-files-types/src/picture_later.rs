//! The commands of the gallery the core does not have yet.
//!
//! Pattern: Builder — as `picture.rs`. All are in `support.rs` as unsupported, from `requirements.md` of the gallery
//! (Required and Optional). The face, label and similarity commands stay local only and off by default when they come
//! (ADR 0029).

use fs_types::{BasicType, CommandDescription, CommandKind};

use crate::common::{command, field, on_record, optional_record, text};

fn on_picture(name: &str, kind: CommandKind) -> CommandDescription {
    on_record("picture", name, kind, "picture", "Picture")
}

fn keywords() -> fs_types::FieldDescription {
    field("keywords", BasicType::Keywords)
}

/// Library: register, import, scan, upload from a phone, derived data.
fn library() -> Vec<CommandDescription> {
    vec![
        command("picture", "register", CommandKind::Create)
            .input(crate::common::record("file", "File")),
        command("picture", "import", CommandKind::Create)
            .input(field("source", BasicType::Link).required()),
        command("picture", "libraryScan", CommandKind::Other)
            .input(optional_record("folder", "Folder")),
        command("picture", "autoUpload", CommandKind::Other).input(text("device")),
        on_picture("metadataRead", CommandKind::Other),
        on_picture("metadataEdit", CommandKind::Change)
            .input(field("takenAt", BasicType::DateTime))
            .input(field("description", BasicType::MultilineText))
            .input(field("place", BasicType::Place)),
        on_picture("metadataExport", CommandKind::Other),
        on_picture("tagAdd", CommandKind::Change).input(keywords()),
        on_picture("tagRemove", CommandKind::Change).input(keywords()),
        on_picture("previewGenerate", CommandKind::Other),
        on_picture("open", CommandKind::Other),
        on_picture("videoPlay", CommandKind::Other),
        on_picture("edit", CommandKind::Change)
            .input(text("rotation"))
            .input(text("crop")),
        on_picture("delete", CommandKind::Delete),
        on_picture("restore", CommandKind::Other),
        command("picture", "duplicates", CommandKind::Find),
        on_picture("merge", CommandKind::Other),
        on_picture("stackCreate", CommandKind::Create),
    ]
}

/// Sharing, places, people, search and the optional ideas.
fn extras() -> Vec<CommandDescription> {
    vec![
        on_picture("shareCreate", CommandKind::Other)
            .input(field("password", BasicType::Text))
            .input(field("expires", BasicType::DateTime))
            .input(text("stripLocation")),
        on_picture("shareGrant", CommandKind::Other).input(text("grantee").required()),
        on_picture("shareRevoke", CommandKind::Delete),
        command("picture", "placeList", CommandKind::Find),
        on_picture("placeAssign", CommandKind::Change).input(field("place", BasicType::Place)),
        on_picture("placeResolve", CommandKind::Other),
        command("picture", "sync", CommandKind::Other),
        command("picture", "personDetect", CommandKind::Other),
        on_picture("personName", CommandKind::Change).input(text("person").required()),
        command("picture", "personMerge", CommandKind::Other)
            .input(text("person").required())
            .input(text("into").required()),
        on_picture("personLink", CommandKind::Other).input(text("contact").required()),
        on_picture("labelSuggest", CommandKind::Other),
        command("picture", "searchSemantic", CommandKind::Find).input(text("text").required()),
        on_picture("similar", CommandKind::Find),
        on_picture("ocr", CommandKind::Other),
        on_picture("favourite", CommandKind::Change).input(text("favourite")),
        on_picture("archive", CommandKind::Change).input(text("archived")),
        on_picture("visibility", CommandKind::Change).input(text("visibility")),
        on_picture("commentAdd", CommandKind::Other)
            .input(field("comment", BasicType::MultilineText).required()),
        on_picture("ratingSet", CommandKind::Change).input(text("rating")),
        command("picture", "batch", CommandKind::Other).input(text("operation").required()),
        command("picture", "story", CommandKind::Other).input(field("from", BasicType::Date)),
        on_picture("watermark", CommandKind::Other).input(text("mark")),
        on_picture("transcode", CommandKind::Other).input(text("codec")),
    ]
}

/// The commands of `requirements.md` (Gallery) that are still to build.
pub(crate) fn later() -> Vec<CommandDescription> {
    let mut all = library();
    all.extend(extras());
    all
}
