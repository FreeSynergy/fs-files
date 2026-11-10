//! Which commands the core answers today.
//!
//! Pattern: Specification (`coding.md` §2) — a fixed table (type, command) of what the core has; everything else in the
//! description is unsupported. Provisional (user rule of 2026-11-10): `types.capnp` has no `supported` field in
//! `CommandDescription` yet (Thread T adds it); until it does, this table is the one place that says it, and the
//! frontend is never changed to switch a control on. When the core gets a command, one row is added here.

use crate::registry::registry;

/// The (type, command) pairs the core answers. Anything else described is unsupported.
pub const SUPPORTED: &[(&str, &str)] = &[
    ("File", "find"),
    ("File", "rename"),
    ("File", "move"),
    ("File", "classify"),
    ("File", "copy"),
    ("File", "delete"),
    ("File", "restore"),
    ("File", "restoreTo"),
    ("File", "purge"),
    ("File", "emptyTrash"),
    ("File", "resolve"),
    ("File", "upload"),
    ("File", "download"),
    ("File", "uploadList"),
    ("File", "uploadAbort"),
    ("Folder", "find"),
    ("Folder", "create"),
    ("Folder", "rename"),
    ("Folder", "move"),
    ("Folder", "delete"),
    ("Folder", "resolve"),
    ("Folder", "rescan"),
    ("StorageBackend", "find"),
    ("StorageBackend", "create"),
    ("StorageBackend", "delete"),
    ("Picture", "find"),
    ("Picture", "download"),
    ("Picture", "sensitiveMark"),
];

/// Does the core answer `command` of `data_type`?
pub fn is_supported(data_type: &str, command: &str) -> bool {
    SUPPORTED.contains(&(data_type, command))
}

/// Every described command the core does not answer, as (type, command).
pub fn unsupported() -> Vec<(String, String)> {
    registry()
        .iter()
        .flat_map(|t| {
            t.commands
                .iter()
                .filter(|c| !is_supported(&t.name, &c.name))
                .map(|c| (t.name.clone(), c.name.clone()))
        })
        .collect()
}
