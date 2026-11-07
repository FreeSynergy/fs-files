//! The real `fs-files-server` on a Unix socket, on a thread of its own, over a given provider.
//!
//! Test helper: starts the thing the tests talk to (the providers, the way to stop it, its thread).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fs_files_core::{Backends, FileFeed, FileRepo, Files, Transfer};
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{bind_socket, serve};

/// The way to stop the server: whoever holds it may, once. The providers may hold it too.
pub type Stop = Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>;

/// What the server is in front of.
pub struct Providers {
    pub files: Arc<dyn Files>,
    pub transfer: Arc<dyn Transfer>,
    pub backends: Arc<dyn Backends>,
    pub feed: Arc<dyn FileFeed>,
}

impl Providers {
    /// All four are this repository.
    pub fn of(repo: &FileRepo) -> Self {
        Self {
            files: Arc::new(repo.clone()),
            transfer: Arc::new(repo.clone()),
            backends: Arc::new(repo.clone()),
            feed: Arc::new(repo.clone()),
        }
    }
}

/// Stop the server now.
pub fn stop(stop: &Stop) {
    if let Some(tx) = stop.lock().unwrap().take() {
        let _ = tx.send(());
    }
}

/// Start the server over `providers`; answers its socket and its thread. It ends when `stopped` fires.
pub fn start(
    dir: &Path,
    providers: Providers,
    stopped: tokio::sync::oneshot::Receiver<()>,
) -> (PathBuf, std::thread::JoinHandle<()>) {
    let socket = dir.join("files.sock");
    let (ready, listening) = std::sync::mpsc::channel();
    let path = socket.clone();
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let bound = bind_socket(&path).await.unwrap();
            let root = FilesServer::new(
                providers.files,
                providers.transfer,
                providers.backends,
                providers.feed,
            );
            ready.send(()).unwrap();
            let shutdown = async {
                let _ = stopped.await;
            };
            serve(bound, root.into_client().client, shutdown)
                .await
                .unwrap();
        });
    });
    listening.recv().unwrap();
    (socket, thread)
}
