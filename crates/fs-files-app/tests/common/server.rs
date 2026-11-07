//! The real `fs-files-server` on a Unix socket, on a thread of its own, over a given provider.
//!
//! Pattern: none — one function that starts the thing the tests talk to.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_files_core::FileRepo;
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{bind_socket, serve};

/// Start the server over `repo`; answers its socket, the way to stop it and its thread.
pub fn start(
    dir: &Path,
    repo: &FileRepo,
) -> (
    PathBuf,
    tokio::sync::oneshot::Sender<()>,
    std::thread::JoinHandle<()>,
) {
    let socket = dir.join("files.sock");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let (ready, listening) = std::sync::mpsc::channel();
    let (path, provider) = (socket.clone(), repo.clone());
    let thread = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let bound = bind_socket(&path).await.unwrap();
            let (a, b, c) = (provider.clone(), provider.clone(), provider.clone());
            let root = FilesServer::new(Arc::new(a), Arc::new(b), Arc::new(c), Arc::new(provider));
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
    (socket, stop, thread)
}
