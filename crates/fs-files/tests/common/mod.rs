//! Shared helpers of the integration tests: a real server on a real Unix socket, over a real
//! `FileRepo` (in-memory store, local folder in a temporary directory).

#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_files_core::{Backends, FileFeed, FileRepo, Files, LocalFolder, Transfer};
use fs_files_server::FilesServer;
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::{bind_socket, connect, serve};
use fs_objects::{Store, TursoStore};

pub mod backend;
pub mod links;
pub mod program;
pub mod search;
pub mod seen;
pub mod stub;

pub const ME: &str = "https://local.invalid/users/owner";

/// What a test sees of the program: the repository, the folder the files live in.
pub struct Fixture {
    pub repo: Arc<FileRepo>,
    pub root: PathBuf,
}

/// A real server in front of a real repository.
pub async fn with_server<F, Fut>(test: F)
where
    F: FnOnce(files::Client, Fixture) -> Fut,
    Fut: Future<Output = ()>,
{
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let root = dir.path().join("files");
    let backend = Arc::new(LocalFolder::open(&root).await.unwrap());
    let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
    let (files, transfer, backends, feed) =
        (repo.clone(), repo.clone(), repo.clone(), repo.clone());
    let fixture = Fixture { repo, root };
    with_provider(files, transfer, backends, feed, |c| test(c, fixture)).await;
}

/// A real server in front of this one repository.
pub async fn with_repo<F, Fut>(repo: Arc<FileRepo>, test: F)
where
    F: FnOnce(files::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let (a, b, c, d) = (repo.clone(), repo.clone(), repo.clone(), repo);
    with_provider(a, b, c, d, test).await;
}

/// A repository over a store file (with a key if `encrypted`, ADR 0024) and a local folder.
pub async fn repo_on_disk(dir: &std::path::Path, encrypted: bool) -> Arc<FileRepo> {
    use fs_objects::{Cipher, EncryptionKey, OpenOptions, StaticKeyProvider};
    let mut opts = OpenOptions::default();
    if encrypted {
        let key = EncryptionKey {
            cipher: Cipher::Aes256Gcm,
            key: vec![7; 32],
        };
        opts.key_provider = Some(Arc::new(StaticKeyProvider(key)));
    }
    let store = TursoStore::open_with(dir.join("store.db"), opts)
        .await
        .unwrap();
    let store: Arc<dyn Store> = Arc::new(store);
    let backend = Arc::new(LocalFolder::open(dir.join("files")).await.unwrap());
    Arc::new(FileRepo::new(store, ME, "main", backend))
}

/// A real server in front of these providers.
pub async fn with_provider<F, Fut>(
    files: Arc<dyn Files>,
    transfer: Arc<dyn Transfer>,
    backends: Arc<dyn Backends>,
    feed: Arc<dyn FileFeed>,
    test: F,
) where
    F: FnOnce(files::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("files.sock");
            let root = FilesServer::new(files, transfer, backends, feed).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<files::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

/// A real server in front of this one stub.
pub async fn with_stub<F, Fut>(stub: stub::Stub, test: F)
where
    F: FnOnce(files::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let stub = Arc::new(stub);
    with_provider(stub.clone(), stub.clone(), stub.clone(), stub, test).await;
}

pub mod client;
pub use client::*;
pub use seen::*;
