//! `Search` interface: files and folders that match a filter.
//!
//! Pattern: Adapter — wire request in, `Files::find_sorted`, wire page out. The store filters and
//! orders; this only reads the filter, the cursor and the sort order.

use std::rc::Rc;
use std::sync::Arc;

use fs_files_core::Files;
use fs_interfaces::files_capnp::search;

use crate::errors::rpc_error;
use crate::wire::entry::write_page;
use crate::wire::{cursor, filter, ids};

pub(crate) struct SearchImpl {
    pub(crate) files: Arc<dyn Files>,
}

impl search::Server for SearchImpl {
    async fn find(
        self: Rc<Self>,
        params: search::FindParams,
        mut results: search::FindResults,
    ) -> Result<(), capnp::Error> {
        let p = params.get()?;
        let filter = filter::read(p.get_filter()?)?;
        let at = cursor::read(p.get_cursor()?.get_opaque()?)?;
        let sort = ids::sort(p.get_sort()?)?;
        let page = self
            .files
            .find_sorted(&filter, at, sort)
            .await
            .map_err(rpc_error)?;
        write_page(results.get().init_page(), &page)
    }
}
