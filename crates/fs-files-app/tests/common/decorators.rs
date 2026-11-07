//! Providers that misbehave on purpose, for the real server to sit in front of.
//!
//! Pattern: Decorator — each wraps the real `FileRepo` and implements the same trait. `Renamed` tells a
//! name the core would never have accepted (a hostile or broken server); `Tripwire` stops the server
//! when the n-th chunk of an upload has arrived (a connection that breaks in the middle of an upload).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use fs_files_core::{
    Blob, Cursor, File, FileRepo, Files, Folder, OnConflict, Page, Result, Transfer, Upload,
    UploadInfo,
};
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;

use super::server::{Stop, stop};

/// `Files` that gives every file this name.
pub struct Renamed(pub FileRepo, pub String);

#[async_trait]
impl Files for Renamed {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    async fn get(&self, id: ObjectId) -> Result<Option<File>> {
        Ok(self.0.get(id).await?.map(|mut f| {
            f.name.clone_from(&self.1);
            f.envelope.name.clone_from(&self.1);
            f
        }))
    }
    async fn get_folder(&self, id: ObjectId) -> Result<Option<Folder>> {
        self.0.get_folder(id).await
    }
    async fn list(&self, folder: Option<ObjectId>, cursor: Option<Cursor>) -> Result<Page> {
        self.0.list(folder, cursor).await
    }
}

/// `Transfer` that stops the server after `after` chunks have been written.
pub struct Tripwire {
    pub repo: FileRepo,
    pub stop: Stop,
    pub after: usize,
    pub written: Arc<AtomicUsize>,
}

struct Counting {
    inner: Arc<dyn Upload>,
    trip: Arc<Tripwire>,
}

#[async_trait]
impl Upload for Counting {
    async fn write(&self, offset: u64, data: &[u8]) -> Result<()> {
        self.inner.write(offset, data).await?;
        if self.trip.written.fetch_add(1, Ordering::SeqCst) + 1 >= self.trip.after {
            stop(&self.trip.stop);
        }
        Ok(())
    }
    async fn received(&self) -> Result<u64> {
        self.inner.received().await
    }
    async fn finish(&self) -> Result<File> {
        self.inner.finish().await
    }
    async fn abort(&self) -> Result<()> {
        self.inner.abort().await
    }
}

/// The `Transfer` of a [`Tripwire`], shared so that its uploads can reach it.
pub struct Tripped(pub Arc<Tripwire>);

#[async_trait]
impl Transfer for Tripped {
    async fn open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        self.0.repo.open(id).await
    }
    async fn start_upload_as(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
        on: OnConflict,
    ) -> Result<(Arc<dyn Upload>, String)> {
        let (up, id) = self
            .0
            .repo
            .start_upload_as(folder, name, media_type, on)
            .await?;
        Ok((
            Arc::new(Counting {
                inner: up,
                trip: self.0.clone(),
            }),
            id,
        ))
    }
    async fn start_upload(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
    ) -> Result<(Arc<dyn Upload>, String)> {
        self.start_upload_as(folder, name, media_type, OnConflict::Refuse)
            .await
    }
    async fn pending_uploads(&self) -> Result<Vec<UploadInfo>> {
        self.0.repo.pending_uploads().await
    }
    async fn resume_upload(&self, id: &str) -> Result<Arc<dyn Upload>> {
        let up = self.0.repo.resume_upload(id).await?;
        Ok(Arc::new(Counting {
            inner: up,
            trip: self.0.clone(),
        }))
    }
}
