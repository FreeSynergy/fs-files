//! `finish` verifies the size reported by the backend.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::names;
use fs_files_core::{BlobBackend, BlobInfo, Error, FileRepo, LocalFolder, Transfer};
use fs_interfaces::Capabilities;
use fs_objects::{Store, TursoStore};

/// A backend whose commit reports one byte too many.
struct Lying(LocalFolder);

#[async_trait]
impl BlobBackend for Lying {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn kind(&self) -> &'static str {
        "local"
    }
    async fn get(&self, k: &str, o: u64, l: u32) -> fs_files_core::Result<Vec<u8>> {
        self.0.get(k, o, l).await
    }
    async fn stat(&self, k: &str) -> fs_files_core::Result<Option<BlobInfo>> {
        self.0.stat(k).await
    }
    async fn put(&self, k: &str, o: u64, d: &[u8]) -> fs_files_core::Result<()> {
        self.0.put(k, o, d).await
    }
    async fn pending(&self, k: &str) -> fs_files_core::Result<u64> {
        self.0.pending(k).await
    }
    async fn delete(&self, k: &str) -> fs_files_core::Result<()> {
        self.0.delete(k).await
    }
    async fn commit(&self, k: &str) -> fs_files_core::Result<BlobInfo> {
        let mut info = self.0.commit(k).await?;
        info.size += 1;
        Ok(info)
    }
}

#[tokio::test]
async fn finish_verifies_the_size_and_creates_nothing_on_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let local = LocalFolder::open(dir.path()).await.unwrap();
    let repo = FileRepo::new(store, common::ME, "main", Arc::new(Lying(local)));
    let (up, _) = repo.start_upload(None, "x", "text/plain").await.unwrap();
    up.write(0, b"abc").await.unwrap();
    assert!(matches!(up.finish().await, Err(Error::Backend(_))));
    assert!(names(&repo, None).await.is_empty());
}
