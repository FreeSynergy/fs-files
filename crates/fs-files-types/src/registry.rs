//! The registry of all types of the program.
//!
//! Pattern: Registry (`coding.md` §2) — `fs-types` `TypeRegistry`, filled from the `DescribesType` implementations.

use fs_types::TypeRegistry;

use fs_types::{CommandKind, DescribesType, TypeDescription};

use crate::{Album, File, Folder, Picture, StorageBackend, is_supported};

/// The description of `T`, each command marked by what the core answers today (`support.rs`).
///
/// `fs-types` reserves `CommandKind::Change` for the `Edit` of the type's own fields (a `change` command has no input,
/// `ChangeWithInput`). The commands here that change a record and ask for input (rename, move, classify, ...) are
/// actions with their own form, so they are `Other` on the wire.
fn described<T: DescribesType>() -> TypeDescription {
    let mut description = T::describe();
    for command in &mut description.commands {
        command.supported = is_supported(&description.name, &command.name);
        if command.kind == CommandKind::Change && !command.input.is_empty() {
            command.kind = CommandKind::Other;
        }
    }
    description
}

/// All data types of the files program (with the gallery facet), validated.
///
/// # Panics
/// If a description is unsound: the tests make sure none is.
pub fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry
        .add(described::<File>())
        .and_then(|r| r.add(described::<Folder>()))
        .and_then(|r| r.add(described::<StorageBackend>()))
        .and_then(|r| r.add(described::<Album>()))
        .and_then(|r| r.add(described::<Picture>()))
        .expect("the descriptions of fs-files are sound");
    registry
}
