//! The world of the tests: the real core, a real local folder in a temporary directory, and a store in
//! memory (or, for marked files, an encrypted one in that directory: the store refuses them otherwise).
//!
//! Pattern: Facade over the real thing. Nothing here pretends to be the core.
#![allow(dead_code)] // each test file uses a different subset

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_components_workbenches::file_manager::{FileManagerWidget, FilesState, PAGE, Variant};
use fs_files_app::{FileAdapter, labels};
use fs_files_core::{File, FileRepo, Files, Folder, LocalFolder, Transfer};
use fs_store::{
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
        let adapter = Arc::new(FileAdapter::new(
            Arc::new(repo.clone()),
            Arc::new(repo.clone()),
            runtime.handle().clone(),
        ));
        Self {
            runtime,
            dir,
            store,
            repo,
            adapter,
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
