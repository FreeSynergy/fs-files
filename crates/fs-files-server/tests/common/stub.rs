//! A provider that answers what the test tells it to: reports longer than a message, a provider
//! without a feed. The server must cope with whatever a provider returns.

use std::sync::Arc;

use async_trait::async_trait;
use fs_files_core::{
    BackendInfo, Backends, Blob, Entry, File, FileFeed, FileListener, Files, Folder, Page,
    RescanReport, Result, Subscription, Transfer, TrashReport, UploadInfo,
};
use fs_interfaces::Capabilities;
use fs_store::{Envelope, ObjectId};

#[derive(Default)]
pub struct Stub {
    pub rescan: RescanReport,
    pub failed: Vec<ObjectId>,
    pub uploads: Vec<UploadInfo>,
    pub feed: Arc<fs_files_core::Registry>,
    pub with_feed: bool,
    /// Reading an entry fails, as a store that went away would.
    pub fail_reads: bool,
}

/// A file with a name of 255 bytes and a long media type: a heavy entry.
pub fn heavy_file(n: usize) -> File {
    let name = format!("{n:08}{}", "x".repeat(247));
    let mut envelope = Envelope::new("Document", name.clone());
    envelope.sensitive = vec!["dpv:Financial".repeat(40); 4];
    File {
        id: envelope.id,
        iri: format!("https://local.invalid/files/{}", envelope.id),
        name,
        media_type: "application/octet-stream".into(),
        size: n as u64,
        checksum: format!("blake3:{}", "0".repeat(64)),
        folder: None,
        trashed: false,
        sensitive: envelope.sensitive.clone(),
        envelope,
    }
}

pub fn heavy_folder(n: usize) -> Folder {
    let envelope = Envelope::new("Collection", format!("{n:08}{}", "d".repeat(247)));
    Folder {
        id: envelope.id,
        iri: String::new(),
        name: envelope.name.clone(),
        parent: None,
        envelope,
    }
}

#[async_trait]
impl Files for Stub {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("stub");
        if self.with_feed {
            c.supported("file.subscribe");
        } else {
            c.unsupported("file.subscribe", "this stub has no feed");
        }
        c
    }
    async fn get(&self, _: ObjectId) -> Result<Option<File>> {
        if self.fail_reads {
            return Err(fs_files_core::Error::Backend("the store went away".into()));
        }
        Ok(None)
    }
    async fn get_folder(&self, _: ObjectId) -> Result<Option<Folder>> {
        Ok(None)
    }
    async fn list(&self, _: Option<ObjectId>, _: Option<fs_files_core::Cursor>) -> Result<Page> {
        Ok(Page {
            items: vec![],
            next: None,
        })
    }
    async fn rescan(&self, _: Option<ObjectId>) -> Result<RescanReport> {
        Ok(self.rescan.clone())
    }
    async fn empty_trash(&self) -> Result<TrashReport> {
        Ok(TrashReport {
            purged: 3,
            failed: self.failed.clone(),
        })
    }
}

#[async_trait]
impl Transfer for Stub {
    async fn open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        Err(fs_files_core::Error::NotFound(format!("file {id}").into()))
    }
    async fn pending_uploads(&self) -> Result<Vec<UploadInfo>> {
        Ok(self.uploads.clone())
    }
}

#[async_trait]
impl Backends for Stub {
    async fn list_backends(&self) -> Result<Vec<BackendInfo>> {
        Ok(vec![])
    }
}

impl FileFeed for Stub {
    fn subscribe(&self, listener: Arc<dyn FileListener>) -> Subscription {
        self.feed.add(listener)
    }
}

pub fn entries(n: usize) -> Vec<Entry> {
    (0..n).map(|i| Entry::File(heavy_file(i))).collect()
}
