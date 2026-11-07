//! `Backends` interface: which providers hold the bytes.
//!
//! Pattern: Adapter — wire request in, `Backends` call, wire response out. `add` and `remove` change
//! what the program holds its bytes in and run to their end. The configuration of `add` can carry
//! credentials: it is read with a limit and never put into an error here.

use std::rc::Rc;
use std::sync::Arc;

use fs_files_core::Backends;
use fs_interfaces::files_capnp::backends;

use crate::errors::{finish, rpc_error};
use crate::wire::count;
use crate::wire::report::write_backend;
use crate::wire::text::{MAX_CONFIG, MAX_SHORT, limited};

pub(crate) struct BackendsImpl {
    pub(crate) backends: Arc<dyn Backends>,
}

type Done = Result<(), capnp::Error>;

impl backends::Server for BackendsImpl {
    async fn list(
        self: Rc<Self>,
        _: backends::ListParams,
        mut results: backends::ListResults,
    ) -> Done {
        let all = self.backends.list_backends().await.map_err(rpc_error)?;
        let mut items = results.get().init_items(count(all.len())?);
        for (i, b) in (0u32..).zip(&all) {
            write_backend(items.reborrow().get(i), b);
        }
        Ok(())
    }

    async fn add(
        self: Rc<Self>,
        params: backends::AddParams,
        mut results: backends::AddResults,
    ) -> Done {
        let config = limited(params.get()?.get_config()?, "config", MAX_CONFIG)?.to_string();
        let backends = self.backends.clone();
        let info = finish(async move { backends.add_backend(&config).await }).await?;
        write_backend(results.get().init_info(), &info);
        Ok(())
    }

    async fn remove(
        self: Rc<Self>,
        params: backends::RemoveParams,
        _: backends::RemoveResults,
    ) -> Done {
        let id = limited(params.get()?.get_id()?, "id", MAX_SHORT)?.to_string();
        let backends = self.backends.clone();
        finish(async move { backends.remove_backend(&id).await }).await
    }
}
