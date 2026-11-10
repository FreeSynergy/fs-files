//! `StorageBackend`: where the bytes of the files are kept (local folder, S3).
//!
//! Pattern: Registry entry (trait first, ADR 0043) — `DescribesType`. Standard: none exists for "a place that holds
//! bytes"; the closest is the AS2 `Service` (an actor that provides something), whose `id` and `name` we use. The
//! core's own contract (`BackendInfo` in `files.capnp`) is the source of the fields. Provisional, for the strategy.

use fs_types::{BasicType, CommandKind, DescribesType, FieldDescription, TypeDescription};

use crate::common::{PROGRAM, command, on_record, text};

/// The `StorageBackend` type.
pub struct StorageBackend;

impl DescribesType for StorageBackend {
    fn describe() -> TypeDescription {
        TypeDescription::new("StorageBackend", PROGRAM, "AS2 Service", "storage")
            .field(text("name").required())
            .field(text("kind").required())
            .field(
                FieldDescription::new("state", BasicType::Choice)
                    .choices(&["active", "idle"])
                    .label("files.state"),
            )
            .command(command("storage", "find", CommandKind::Find))
            .command(
                command("storage", "create", CommandKind::Create)
                    // JSON text of the backend configuration (`kind`, path or bucket, credentials); never logged.
                    .input(crate::common::field("config", BasicType::MultilineText).required()),
            )
            .command(on_record(
                "storage",
                "delete",
                CommandKind::Delete,
                "storage",
                "StorageBackend",
            ))
            .command(on_record(
                "storage",
                "quota",
                CommandKind::Other,
                "storage",
                "StorageBackend",
            ))
    }
}
