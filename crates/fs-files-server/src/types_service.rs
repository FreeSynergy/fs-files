//! `Files.types`: the catalog of the type descriptions.
//!
//! Pattern: Adapter — the generated `TypeCatalog` server over the registry of `fs-files-types`; it writes the
//! descriptions and holds no logic of its own.

use std::rc::Rc;

use fs_files_types::{registry, write_catalog};
use fs_interfaces::types_capnp::type_catalog;

/// Serves the descriptions of this program.
pub struct CatalogImpl;

impl type_catalog::Server for CatalogImpl {
    async fn describe(
        self: Rc<Self>,
        _: type_catalog::DescribeParams,
        mut results: type_catalog::DescribeResults,
    ) -> Result<(), capnp::Error> {
        let registry = registry();
        let n = u32::try_from(registry.len()).unwrap_or(u32::MAX);
        write_catalog(&registry, results.get().init_types(n));
        Ok(())
    }
}
