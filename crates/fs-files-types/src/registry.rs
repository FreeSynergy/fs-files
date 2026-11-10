//! The registry of all types of the program.
//!
//! Pattern: Registry (`coding.md` §2) — `fs-types` `TypeRegistry`, filled from the `DescribesType` implementations.

use fs_types::TypeRegistry;

use crate::{Album, File, Folder, Picture, StorageBackend};

/// All data types of the files program (with the gallery facet), validated.
///
/// # Panics
/// If a description is unsound: the tests make sure none is.
pub fn registry() -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry
        .register::<File>()
        .and_then(|r| r.register::<Folder>())
        .and_then(|r| r.register::<StorageBackend>())
        .and_then(|r| r.register::<Album>())
        .and_then(|r| r.register::<Picture>())
        .expect("the descriptions of fs-files are sound");
    registry
}
