//! `Transfer` over the socket, and the upload and the blob it hands out.
//!
//! Pattern: Adapter — the two handles are small adapters too: an `RpcUpload` is the id of a session and the connection;
//! an `RpcBlob` is the id of a file and the connection. Each call names its upload or file again on the
//! server (`resumeUpload`, `open`), because a capability of the wire cannot leave the thread of the
//! connection and an upload is meant to be taken up again by its id. The cost is one more round trip
//! per call on a local socket; the gain is that a handle survives a dropped one and nothing leaks.
//! Bytes go by the chunks the caller gives (the server takes at most 16 MiB per `write` and answers at
//! most 32 MiB per `read`: a larger one is refused or cut by the server, see `DESIGN.md`).

use std::sync::Arc;

use async_trait::async_trait;
use fs_files_core::{Blob, File, OnConflict, Result, Transfer, Upload, UploadInfo};
use fs_objects::ObjectId;

use crate::peer::Link;
use crate::{RpcFiles, transfer_calls as calls};

/// One resumable upload, by its id.
struct RpcUpload {
    link: Arc<Link>,
    id: String,
}

/// The bytes of one file, by its id.
struct RpcBlob {
    link: Arc<Link>,
    id: ObjectId,
}

#[async_trait]
impl Transfer for RpcFiles {
    async fn open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        // Opened once to learn whether the file can be read; every read opens it again.
        self.call(move |r| async move { calls::open(&r.transfer, id).await.map(|_| ()) })
            .await?;
        Ok(Arc::new(RpcBlob {
            link: self.link(),
            id,
        }))
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

    async fn start_upload_as(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
        on: OnConflict,
    ) -> Result<(Arc<dyn Upload>, String)> {
        let (name, media_type) = (name.to_string(), media_type.to_string());
        let id =
            self.call(move |r| async move {
                calls::start(&r.transfer, folder, name, media_type, on).await
            })
            .await?;
        Ok((
            Arc::new(RpcUpload {
                link: self.link(),
                id: id.clone(),
            }),
            id,
        ))
    }

    async fn pending_uploads(&self) -> Result<Vec<UploadInfo>> {
        self.call(move |r| async move { calls::pending(&r.transfer).await })
            .await
    }

    async fn resume_upload(&self, id: &str) -> Result<Arc<dyn Upload>> {
        let id = id.to_string();
        let asked = id.clone();
        // Taken up once to learn whether the upload exists; every call takes it up again.
        self.call(move |r| async move { calls::resume(&r.transfer, asked).await.map(|_| ()) })
            .await?;
        Ok(Arc::new(RpcUpload {
            link: self.link(),
            id,
        }))
    }
}

impl RpcUpload {
    async fn on_upload<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(fs_interfaces::files_capnp::upload::Client) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + 'static,
    {
        let id = self.id.clone();
        self.link
            .call(move |r| async move {
                let upload = calls::resume(&r.transfer, id).await?;
                work(upload).await
            })
            .await
    }
}

#[async_trait]
impl Upload for RpcUpload {
    async fn write(&self, offset: u64, data: &[u8]) -> Result<()> {
        let data = data.to_vec();
        self.on_upload(move |u| async move { calls::write_at(&u, offset, data).await })
            .await
    }

    async fn received(&self) -> Result<u64> {
        self.on_upload(|u| async move { calls::received(&u).await })
            .await
    }

    async fn finish(&self) -> Result<File> {
        self.on_upload(|u| async move { calls::finish(&u).await })
            .await
    }

    async fn abort(&self) -> Result<()> {
        self.on_upload(|u| async move { calls::abort(&u).await })
            .await
    }
}

impl RpcBlob {
    async fn on_blob<T, F, Fut>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(fs_interfaces::common_capnp::blob::Client) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + 'static,
    {
        let id = self.id;
        self.link
            .call(move |r| async move {
                let blob = calls::open(&r.transfer, id).await?;
                work(blob).await
            })
            .await
    }
}

#[async_trait]
impl Blob for RpcBlob {
    async fn size(&self) -> Result<(u64, String)> {
        self.on_blob(|b| async move { calls::size(&b).await }).await
    }

    async fn read(&self, offset: u64, length: u32) -> Result<Vec<u8>> {
        self.on_blob(move |b| async move { calls::read_range(&b, offset, length).await })
            .await
    }
}
