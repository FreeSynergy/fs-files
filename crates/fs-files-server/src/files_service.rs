//! `Files` root interface: support report, the six capabilities, subscribe.
//!
//! Pattern: Adapter + Factory — the root hands out `Folders`, `Transfer`, `Search` and `Backends`.
//! [`FilesServer`] is what a program serves; it depends on the four traits, never on `FileRepo`
//! (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_files_core::{Backends, FileFeed, Files, Transfer};
use fs_interfaces::Unsupported;
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::{unsupported, write_uuid};

use crate::backends_service::BackendsImpl;
use crate::errors::rpc_error;
use crate::feed::{offered, subscribe};
use crate::folders_service::FoldersImpl;
use crate::search_service::SearchImpl;
use crate::transfer_service::TransferImpl;
use crate::wire::CONTRACT_VERSION;

/// The root capability of the files program.
pub struct FilesServer {
    files: Arc<dyn Files>,
    transfer: Arc<dyn Transfer>,
    backends: Arc<dyn Backends>,
    feed: Arc<dyn FileFeed>,
}

impl FilesServer {
    /// Serve these providers; a program passes its one `FileRepo` for all four.
    pub fn new(
        files: Arc<dyn Files>,
        transfer: Arc<dyn Transfer>,
        backends: Arc<dyn Backends>,
        feed: Arc<dyn FileFeed>,
    ) -> Self {
        Self {
            files,
            transfer,
            backends,
            feed,
        }
    }

    /// The capability to hand to `fs_interfaces::rpc::serve`.
    #[must_use]
    pub fn into_client(self) -> files::Client {
        new_client(self)
    }
}

type Done = Result<(), capnp::Error>;

impl files::Server for FilesServer {
    async fn support_report(
        self: Rc<Self>,
        _: files::SupportReportParams,
        mut results: files::SupportReportResults,
    ) -> Done {
        let report = self.files.capabilities();
        let mut out = results.get().init_report();
        report.write(CONTRACT_VERSION, out.reborrow());
        // The store's id, as the store says it: empty where the provider has no store.
        write_uuid(
            out.init_instance(),
            self.files.identity().map(|id| id.as_uuid()),
        );
        Ok(())
    }

    async fn folders(
        self: Rc<Self>,
        _: files::FoldersParams,
        mut r: files::FoldersResults,
    ) -> Done {
        let files = self.files.clone();
        r.get().set_folders(new_client(FoldersImpl { files }));
        Ok(())
    }

    async fn transfer(
        self: Rc<Self>,
        _: files::TransferParams,
        mut r: files::TransferResults,
    ) -> Done {
        let transfer = self.transfer.clone();
        r.get().set_transfer(new_client(TransferImpl { transfer }));
        Ok(())
    }

    async fn search(self: Rc<Self>, _: files::SearchParams, mut r: files::SearchResults) -> Done {
        let files = self.files.clone();
        r.get().set_search(new_client(SearchImpl { files }));
        Ok(())
    }

    async fn backends(
        self: Rc<Self>,
        _: files::BackendsParams,
        mut r: files::BackendsResults,
    ) -> Done {
        let backends = self.backends.clone();
        r.get().set_backends(new_client(BackendsImpl { backends }));
        Ok(())
    }

    /// `Sharing` has no methods yet in the contract: say so instead of handing out an empty capability.
    async fn sharing(self: Rc<Self>, _: files::SharingParams, _: files::SharingResults) -> Done {
        match self.files.sharing().await {
            Err(e) => Err(rpc_error(e)),
            Ok(()) => Err(unsupported(&Unsupported::new(
                "share.create",
                "Sharing is a placeholder in the contract; not built yet, it is possible",
            ))),
        }
    }

    async fn subscribe(
        self: Rc<Self>,
        params: files::SubscribeParams,
        mut results: files::SubscribeResults,
    ) -> Done {
        offered(self.files.as_ref())?;
        let listener = params.get()?.get_listener()?;
        let subscription = subscribe(&self.feed, self.files.clone(), listener);
        results.get().set_subscription(subscription);
        Ok(())
    }
}
