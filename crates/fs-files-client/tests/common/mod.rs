//! Shared set-up of the tests: the real server on a real Unix socket over the real `FileRepo` with a
//! real folder as backend, on a thread of its own, and the client connected to it. Nothing here stands
//! in for either.
//!
//! Pattern: Facade over the set-up.
#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_files_client::RpcFiles;
use fs_files_core::{File, FileRepo, Files, Folder, LocalFolder, Transfer};
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_store::{ObjectId, Store, TursoStore};
use tokio::sync::oneshot;

pub const ME: &str = "https://local.invalid/users/owner";

/// A server running until this is dropped.
pub struct Served {
    pub socket: PathBuf,
    /// The provider behind the server, for set-up and for comparing (never what is under test).
    pub repo: Arc<FileRepo>,
    /// The folder the files live in.
    pub root: PathBuf,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    _dir: tempfile::TempDir,
}

impl Served {
    /// Over a fresh in-memory store and an empty folder.
    pub async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
        let root = dir.path().join("files");
        let backend = Arc::new(LocalFolder::open(&root).await.unwrap());
        let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
        let socket = dir.path().join("files.sock");
        let (stop, stopped) = oneshot::channel::<()>();
        let (ready, listening) = std::sync::mpsc::channel();
        let (path, provider) = (socket.clone(), repo.clone());
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            tokio::task::LocalSet::new().block_on(&runtime, async move {
                let bound = bind_socket(&path).await.unwrap();
                let root = FilesServer::new(
                    provider.clone(),
                    provider.clone(),
                    provider.clone(),
                    provider,
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
        Self {
            socket,
            repo,
            root,
            stop: Some(stop),
            thread: Some(thread),
            _dir: dir,
        }
    }

    /// A client of this server.
    pub fn client(&self) -> RpcFiles {
        RpcFiles::connect(&self.socket).unwrap()
    }

    /// Stop the server; clients stay and find it gone.
    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }

    /// A file made on the provider behind the server (set-up, not under test).
    pub async fn file(&self, folder: Option<ObjectId>, name: &str, data: &[u8]) -> File {
        let (up, _) = self.repo.start_upload(folder, name, "").await.unwrap();
        up.write(0, data).await.unwrap();
        up.finish().await.unwrap()
    }

    /// A folder made on the provider behind the server.
    pub async fn folder(&self, parent: Option<ObjectId>, name: &str) -> Folder {
        self.repo.create_folder(parent, name).await.unwrap()
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        self.stop();
    }
}
