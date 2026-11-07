//! The world of the tests: the real `fs-files-server` on a real Unix socket over the real `FileRepo`, a real
//! local folder in a temporary directory, and a store in memory (or, for marked files, an encrypted one in
//! that directory: the store refuses them otherwise). The app's adapter reaches it through the real
//! `RpcFiles`; `repo` is the provider behind the server, for set-up and for comparing only.
//!
//! Pattern: Facade over the real thing. Nothing here pretends to be the core or the server.
#![allow(dead_code)] // each test file uses a different subset

mod decorators;
mod server;

#[allow(unused_imports)] // each test file uses a different subset
pub use decorators::Renamed;
#[allow(unused_imports)]
pub use decorators::{Tripped, Tripwire};
pub use server::Providers;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_components_workbenches::file_manager::{FileManagerWidget, FilesState, PAGE, Variant};
use fs_files_app::{FileAdapter, labels};
use fs_files_client::RpcFiles;
use fs_files_core::{File, FileRepo, Files, Folder, LocalFolder, Transfer};
use fs_objects::{
    Cipher, EncryptionKey, ObjectId, OpenOptions, StaticKeyProvider, Store, TursoStore,
};

pub const ME: &str = "https://local.invalid/users/owner";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Memory,
    Encrypted,
    PlainFile,
}

pub struct World {
    pub runtime: tokio::runtime::Runtime,
    pub dir: tempfile::TempDir,
    pub store: Arc<dyn Store>,
    pub repo: FileRepo,
    pub adapter: Arc<FileAdapter>,
    /// The socket of the server, for a program that is started as a process.
    pub socket: PathBuf,
    stop: server::Stop,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for World {
    fn drop(&mut self) {
        // The client first: the server stops when its last connection has gone.
        server::stop(&self.stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::with(Kind::Memory)
    }

    /// With a store file under a key, as a person's would be for files marked sensitive.
    pub fn encrypted() -> Self {
        Self::with(Kind::Encrypted)
    }

    /// With a store file without a key: it refuses marks (ADR 0029).
    pub fn plain_file() -> Self {
        Self::with(Kind::PlainFile)
    }

    fn with(kind: Kind) -> Self {
        Self::build(kind, |repo, _| Providers::of(repo))
    }

    /// The server in front of providers the test makes from the repository (a decorator that
    /// misbehaves on purpose); the second argument stops the server.
    pub fn behind(make: impl FnOnce(&FileRepo, &server::Stop) -> Providers) -> Self {
        Self::build(Kind::Memory, make)
    }

    fn build(kind: Kind, make: impl FnOnce(&FileRepo, &server::Stop) -> Providers) -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store: Arc<dyn Store> = runtime.block_on(async {
            if kind == Kind::Encrypted {
                let options = OpenOptions {
                    key_provider: Some(Arc::new(StaticKeyProvider(EncryptionKey {
                        cipher: Cipher::Aes256Gcm,
                        key: vec![7; 32],
                    }))),
                    ..OpenOptions::default()
                };
                Arc::new(
                    TursoStore::open_with(dir.path().join("store.db"), options)
                        .await
                        .unwrap(),
                ) as Arc<dyn Store>
            } else if kind == Kind::PlainFile {
                Arc::new(TursoStore::open(dir.path().join("store.db")).await.unwrap())
                    as Arc<dyn Store>
            } else {
                Arc::new(TursoStore::open_in_memory().await.unwrap()) as Arc<dyn Store>
            }
        });
        let folder = runtime
            .block_on(LocalFolder::open(dir.path().join("root")))
            .unwrap();
        let repo = FileRepo::new(store.clone(), ME, "local", Arc::new(folder)).with_page_size(PAGE);
        let (tx, rx) = tokio::sync::oneshot::channel();
        let stop: server::Stop = Arc::new(std::sync::Mutex::new(Some(tx)));
        let (socket, thread) = server::start(dir.path(), make(&repo, &stop), rx);
        let client = Arc::new(RpcFiles::connect(&socket).unwrap());
        let adapter = Arc::new(FileAdapter::new(
            client.clone(),
            client,
            runtime.handle().clone(),
        ));
        Self {
            runtime,
            dir,
            store,
            repo,
            adapter,
            socket,
            stop,
            thread: Some(thread),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().join("root").canonicalize().unwrap()
    }

    /// A folder outside the root, for what comes from or goes to the person's computer.
    pub fn outside(&self) -> PathBuf {
        let path = self.dir.path().join("outside");
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    pub fn state(&self) -> FilesState {
        FilesState::new(self.adapter.clone(), labels(), Variant::Full)
    }

    pub fn widget(&self) -> FileManagerWidget {
        FileManagerWidget::new(self.adapter.clone(), labels())
    }

    pub fn file(&self, folder: Option<ObjectId>, name: &str, data: &[u8]) -> File {
        self.runtime.block_on(async {
            let (up, _) = self.repo.start_upload(folder, name, "").await.unwrap();
            up.write(0, data).await.unwrap();
            up.finish().await.unwrap()
        })
    }

    pub fn folder(&self, parent: Option<ObjectId>, name: &str) -> Folder {
        self.runtime
            .block_on(self.repo.create_folder(parent, name))
            .unwrap()
    }

    pub fn mark(&self, id: ObjectId) {
        self.runtime
            .block_on(self.repo.classify(id, vec!["dpv:Health".into()]))
            .unwrap();
    }

    /// What the core says this file is now.
    pub fn get(&self, id: ObjectId) -> Option<File> {
        self.runtime.block_on(self.repo.get(id)).unwrap()
    }

    /// Everything below the root except the program's own folder, as relative paths.
    pub fn tree(&self) -> Vec<String> {
        fn walk(base: &Path, at: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                let rel = path.strip_prefix(base).unwrap().display().to_string();
                if rel == ".fs-files" {
                    continue;
                }
                out.push(rel);
                if path.is_dir() && !path.is_symlink() {
                    walk(base, &path, out);
                }
            }
        }
        let mut out = Vec::new();
        let root = self.root();
        walk(&root, &root, &mut out);
        out.sort();
        out
    }
}
