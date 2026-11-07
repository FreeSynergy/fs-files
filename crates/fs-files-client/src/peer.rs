//! The connection: what a frontend holds.
//!
//! Pattern: Adapter — `Files` and `Transfer` of the core in front of the Cap'n Proto client of
//! `files.capnp`. Why: the frontend keeps depending on the traits it already uses, and only the provider
//! behind them changes. The thread of the connection is the shared `Worker` of `fs-interfaces`
//! (`remote.rs` says what it fetches at connect); the calls are in `folder_calls.rs`, `transfer_calls.rs`
//! and `file_calls.rs`; the traits in `files_impl.rs` and `transfer_impl.rs`. Interface first
//! (ADR 0043): `connect` was written down, and the tests written against the traits, before this.

use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::Arc;

use fs_files_core::{Error, Result};
use fs_interfaces::Capabilities;
use fs_interfaces::rpc::client::{Timeouts, Worker};

use crate::remote::FilesRemote;

/// What the server can do and this client does not call: reported as not possible, with the reason,
/// so `capabilities()` never promises more than the methods of `RpcFiles` keep.
const NOT_OFFERED: [&str; 5] = [
    "file.classify",
    "file.subscribe",
    "backend.list",
    "backend.add",
    "backend.remove",
];

/// The report of the server, cut to what this client implements.
fn offered(reported: &Capabilities) -> Capabilities {
    let mut caps = Capabilities::new(reported.provider());
    for entry in reported.entries() {
        if entry.supported && NOT_OFFERED.contains(&entry.capability.as_str()) {
            caps.unsupported(
                entry.capability.as_str(),
                "the server offers it, the client of the files program does not yet",
            );
        } else if entry.supported {
            caps.supported(entry.capability.as_str());
        } else {
            caps.unsupported(entry.capability.as_str(), entry.reason.as_str());
        }
    }
    caps
}

/// The files program as seen over its socket. Implements `Files` and `Transfer`.
///
/// Calls from any thread are fine; they queue for the one thread that owns the connection and
/// overlap there. Dropping it closes the connection, unless an upload or a blob it handed out is
/// still held: they keep the connection until they are dropped.
pub struct RpcFiles {
    link: Arc<Link>,
    capabilities: Capabilities,
}

/// The thread of the connection. Shared with the uploads and blobs handed out, which go on over it
/// (each call names the upload or the file again, so no capability has to leave the thread).
pub(crate) struct Link {
    worker: Worker<FilesRemote>,
}

impl Link {
    /// Run `work` on the connection's thread and wait for its answer.
    pub(crate) async fn call<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(FilesRemote) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + 'static,
    {
        self.worker
            .call(work)
            .await
            .map_err(|fault| Error::Backend(fault.to_string()))?
    }
}

impl RpcFiles {
    /// Connect to the `fs-files-server` that listens on `socket` and ask what it can do.
    ///
    /// Blocks the calling thread until the server has answered; call it before the window opens, not
    /// from inside an async task of a single-threaded runtime.
    ///
    /// # Errors
    /// Nothing listens there, the server runs as another user, it does not answer the contract, or it
    /// stays silent for 5 s (`Timeouts::connect`). A call waits at most 30 s (`Timeouts::call`).
    pub fn connect(socket: &Path) -> io::Result<Self> {
        Self::connect_with(socket, Timeouts::default())
    }

    /// [`connect`](Self::connect) with other deadlines than the defaults (connect 5 s, call 30 s).
    ///
    /// # Errors
    /// As for [`connect`](Self::connect), and the server did not answer in time.
    pub fn connect_with(socket: &Path, timeouts: Timeouts) -> io::Result<Self> {
        let (worker, reported) = Worker::start("files", socket, timeouts)?;
        Ok(Self {
            link: Arc::new(Link { worker }),
            capabilities: offered(&reported),
        })
    }

    /// What the server reported when this connection was made, cut to what the client calls. A
    /// snapshot: the server is not asked again.
    pub(crate) fn report(&self) -> Capabilities {
        self.capabilities.clone()
    }

    /// The connection, to hand to an upload or a blob.
    pub(crate) fn link(&self) -> Arc<Link> {
        self.link.clone()
    }

    /// Run `work` on the connection's thread and wait for its answer.
    pub(crate) async fn call<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(FilesRemote) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + 'static,
    {
        self.link.call(work).await
    }
}
