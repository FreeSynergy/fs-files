//! Shared helpers of the integration tests.

#![allow(dead_code)] // each test file uses a different subset

pub mod disk;
#[cfg(feature = "s3")]
pub mod fake_s3;
pub mod stream;
pub mod suite;

use std::sync::Arc;

use fs_files_core::{File, FileRepo, Files, LocalFolder, Transfer};
use fs_store::{ObjectId, Store, TursoStore};

pub const ME: &str = "https://local.invalid/users/owner";

/// A repository over an in-memory store and a local folder in a temporary directory.
pub struct Env {
    pub repo: FileRepo,
    pub store: Arc<dyn Store>,
    pub dir: tempfile::TempDir,
}

pub async fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let backend = Arc::new(LocalFolder::open(dir.path().join("blobs")).await.unwrap());
    let repo = FileRepo::new(store.clone(), ME, "main", backend);
    Env { repo, store, dir }
}

/// Upload `data` as `name` in one go.
pub async fn upload(
    repo: &FileRepo,
    folder: Option<ObjectId>,
    name: &str,
    media_type: &str,
    data: &[u8],
) -> File {
    let (up, _) = repo.start_upload(folder, name, media_type).await.unwrap();
    up.write(0, data).await.unwrap();
    up.finish().await.unwrap()
}

/// All bytes of a file, read through the `Blob`.
pub async fn read_all(repo: &FileRepo, id: ObjectId) -> Vec<u8> {
    let blob = repo.open(id).await.unwrap();
    let (size, _) = blob.size().await.unwrap();
    let mut out = Vec::new();
    while (out.len() as u64) < size {
        let part = blob.read(out.len() as u64, 65_536).await.unwrap();
        assert!(!part.is_empty());
        out.extend(part);
    }
    out
}

/// Names of the entries of a folder, in order.
pub async fn names(repo: &FileRepo, folder: Option<ObjectId>) -> Vec<String> {
    let page = repo.list(folder, None).await.unwrap();
    page.items
        .into_iter()
        .map(|e| match e {
            fs_files_core::Entry::File(f) => f.name,
            fs_files_core::Entry::Folder(f) => f.name,
        })
        .collect()
}

/// A repository over a store *file* and a blob folder inside `dir`; can be opened again later.
/// `encrypted` opens the store with a key (ADR 0024); otherwise sensitive objects are refused (ADR 0029).
pub async fn reopen(dir: &std::path::Path, encrypted: bool) -> (FileRepo, Arc<dyn Store>) {
    use fs_store::{Cipher, EncryptionKey, OpenOptions, StaticKeyProvider};
    let mut opts = OpenOptions::default();
    if encrypted {
        let key = EncryptionKey {
            cipher: Cipher::Aes256Gcm,
            key: vec![7; 32],
        };
        opts.key_provider = Some(Arc::new(StaticKeyProvider(key)));
    }
    let store: Arc<dyn Store> = Arc::new(
        TursoStore::open_with(dir.join("store.db"), opts)
            .await
            .unwrap(),
    );
    let backend = Arc::new(LocalFolder::open(dir.join("blobs")).await.unwrap());
    (FileRepo::new(store.clone(), ME, "main", backend), store)
}
