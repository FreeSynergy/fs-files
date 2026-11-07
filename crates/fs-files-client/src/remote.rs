//! What the files client takes from the root of the server.
//!
//! Pattern: Adapter — the root `files` of `files.capnp` to the capabilities the calls use (`Folders`,
//! `Transfer`, `Search`), in the form the shared `Worker` asks for ([`Remote`]). `Backends` and
//! `subscribe` are not fetched: nothing here calls them.

use fs_interfaces::Capabilities;
use fs_interfaces::files_capnp::{files, folders, search, transfer};
use fs_interfaces::rpc::client::Remote;

/// The capabilities of the files server a call may use.
#[derive(Clone)]
pub(crate) struct FilesRemote {
    pub(crate) folders: folders::Client,
    pub(crate) transfer: transfer::Client,
    pub(crate) search: search::Client,
}

impl Remote for FilesRemote {
    type Root = files::Client;

    async fn open(root: files::Client) -> Result<(Self, Capabilities), capnp::Error> {
        let report = root.support_report_request().send().promise.await?;
        let capabilities = Capabilities::read(report.get()?.get_report()?)?;
        let folders = root.folders_request().send().promise.await?;
        let transfer = root.transfer_request().send().promise.await?;
        let search = root.search_request().send().promise.await?;
        Ok((
            Self {
                folders: folders.get()?.get_folders()?,
                transfer: transfer.get()?.get_transfer()?,
                search: search.get()?.get_search()?,
            },
            capabilities,
        ))
    }
}
