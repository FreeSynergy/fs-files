//! The commands of `File` the core does not have yet; the description carries them anyway (user rule of 2026-11-10).
//!
//! Pattern: Builder — as `file.rs`. Every command here is in `support.rs` as unsupported: a frontend shows its control
//! locked ("kommt noch") until the core reports it.

use fs_types::{BasicType, CommandDescription, CommandKind};

use crate::common::{command, field, on_record, optional_record, text};

fn on_file(name: &str, kind: CommandKind) -> CommandDescription {
    on_record("file", name, kind, "file", "File")
}

/// The commands of `requirements.md` (Files) that are still to build.
pub(crate) fn later() -> Vec<CommandDescription> {
    vec![
        on_file("preview", CommandKind::Other),
        command("file", "nameCheck", CommandKind::Check).input(text("name").required()),
        command("file", "renameBulk", CommandKind::Other)
            .input(text("pattern").required())
            .input(text("replacement")),
        command("file", "trashConfigure", CommandKind::Other)
            .input(field("retentionDays", BasicType::Text)),
        on_file("shareCreate", CommandKind::Other)
            .input(field("password", BasicType::Text))
            .input(field("expires", BasicType::DateTime))
            .input(text("role")),
        on_file("shareRevoke", CommandKind::Delete),
        on_file("shareRole", CommandKind::Change).input(text("role")),
        on_file("versions", CommandKind::Other),
        on_file("revert", CommandKind::Other).input(text("version").required()),
        on_file("pin", CommandKind::Other).input(text("version").required()),
        on_file("encrypt", CommandKind::Other),
        command("file", "syncConfigure", CommandKind::Other).input(text("peer")),
        on_file("tag", CommandKind::Change).input(field("tags", BasicType::Keywords)),
        command("file", "duplicates", CommandKind::Other),
        command("file", "searchContent", CommandKind::Find).input(text("text").required()),
        on_file("link", CommandKind::Other).input(optional_record("folder", "Folder")),
        command("file", "searchSave", CommandKind::Create)
            .input(text("name").required())
            .input(text("query")),
        on_file("lock", CommandKind::Other),
        command("file", "fileRequest", CommandKind::Create)
            .input(optional_record("folder", "Folder"))
            .input(field("expires", BasicType::DateTime)),
        command("file", "undo", CommandKind::Other),
    ]
}
