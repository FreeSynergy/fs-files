//! Upload sessions: start, resume, write, ask how much arrived, abort.
//!
//! Pattern: State — a session is an object in the store (`Upload`, written once at the start) plus
//! the pending bytes in the backend. The position is *not* stored separately: it is what the backend
//! holds (`BlobBackend::pending`), so it cannot disagree with the bytes and survives any restart.

use std::sync::Arc;

use async_trait::async_trait;
use fs_objects::{ObjectId, Query, StoredObject};
use serde::{Deserialize, Serialize};

use super::blob::new_key;
use super::consts::CONTEXT;
use super::handle::FileRepo;
use super::names::{check_name, numbered_name};
use crate::error::{Error, Result};
use crate::model::OnConflict;
use crate::note::{Kind, Note};
use crate::transfer::Upload;

/// What a session remembers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionRecord {
    pub folder: Option<ObjectId>,
    pub name: String,
    pub media_type: String,
    pub backend: String,
    pub key: String,
    /// Keep both: if the name is taken when the upload finishes, take a free one instead of failing.
    #[serde(default)]
    pub keep_both: bool,
}

/// One running upload (`Upload` in the contract).
pub(super) struct UploadSession {
    pub(super) repo: FileRepo,
    pub(super) id: ObjectId,
    pub(super) rec: SessionRecord,
}

impl FileRepo {
    pub(super) async fn do_start_upload(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
        on: OnConflict,
    ) -> Result<(Arc<dyn Upload>, String)> {
        check_name(name)?;
        self.require_folder(folder).await?;
        let (backend_id, backend) = self.backends.active();
        let name = &self.upload_name(&*backend, folder, name, on).await?;
        let probe = ObjectId::new();
        let rec = SessionRecord {
            folder,
            name: name.clone(),
            media_type: media_type.trim().to_ascii_lowercase(),
            backend: backend_id,
            key: new_key(probe),
            keep_both: on == OnConflict::KeepBoth,
        };
        let o = self.session_object(name, &rec);
        let id = o.envelope.id;
        self.save(o).await?;
        if let Err(e) = backend.put(&rec.key, 0, &[]).await {
            self.objects.delete(id, &self.actor).await?;
            return Err(e);
        }
        let session = UploadSession {
            repo: self.clone(),
            id,
            rec,
        };
        Ok((Arc::new(session), id.to_string()))
    }

    /// The name the upload will have: the given one, or by `on` the first free one (in the records and on the disk).
    /// A name the disk refuses fails here, with nothing created.
    async fn upload_name(
        &self,
        backend: &dyn crate::backend::BlobBackend,
        folder: Option<ObjectId>,
        name: &str,
        on: OnConflict,
    ) -> Result<String> {
        let mut chosen = name.to_string();
        for n in 2..1000 {
            let free = async {
                self.require_free(folder, &chosen, None).await?;
                self.check_target(backend, folder, &chosen).await
            };
            match free.await {
                Err(Error::Conflict(_)) if on == OnConflict::KeepBoth => {
                    chosen = numbered_name(name, n);
                }
                other => return other.map(|()| chosen),
            }
        }
        Err(Error::Conflict("no free name to keep both".into()))
    }

    pub(super) async fn do_resume_upload(&self, id: &str) -> Result<Arc<dyn Upload>> {
        let id: ObjectId = id.parse().map_err(|_| Error::missing(Kind::Upload, id))?;
        let o = self.session_obj(id).await?;
        let rec = serde_json::from_value(o.content)?;
        Ok(Arc::new(UploadSession {
            repo: self.clone(),
            id,
            rec,
        }))
    }

    /// Keys of the pending blobs of every unfinished upload on `backend`.
    pub(super) async fn session_keys(&self, backend: &str) -> Result<Vec<String>> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("kind", "upload")
            .with_index("backend", backend);
        let mut keys = Vec::new();
        for o in self.reads.query(&q).await? {
            let rec: SessionRecord = serde_json::from_value(o.content)?;
            keys.push(rec.key);
        }
        Ok(keys)
    }

    pub(super) async fn session_obj(&self, id: ObjectId) -> Result<StoredObject> {
        match self.objects.get(id).await? {
            Some(o) if o.index.get("kind").is_some_and(|k| k == &["upload"]) => Ok(o),
            _ => Err(Error::missing(Kind::Upload, id)),
        }
    }
}

#[async_trait]
impl Upload for UploadSession {
    async fn write(&self, offset: u64, data: &[u8]) -> Result<()> {
        self.repo.session_obj(self.id).await?; // finished or aborted sessions take nothing
        let backend = self.repo.backends.get(&self.rec.backend)?;
        if offset == 0 && self.committed_size(&*backend).await?.is_some() {
            return Err(Error::Conflict(
                Note::from("all bytes of this upload are in place; call finish")
                    .about(Kind::Upload, self.id),
            ));
        }
        backend.put(&self.rec.key, offset, data).await
    }

    async fn received(&self) -> Result<u64> {
        let backend = self.repo.backends.get(&self.rec.backend)?;
        // After a commit whose record could not be written, the bytes are no longer pending but
        // committed: report their size, so a client does not start over on top of them.
        if let Some(size) = self.committed_size(&*backend).await? {
            return Ok(size);
        }
        backend.pending(&self.rec.key).await
    }

    async fn finish(&self) -> Result<crate::model::File> {
        self.repo.finish_upload(self.id, &self.rec).await
    }

    async fn abort(&self) -> Result<()> {
        // Same order as finish (upload, then the global lock), and never in the middle of a rescan.
        let _one = self.repo.uploads.lock(&self.id.to_string()).await;
        let _all = self.repo.lock.lock().await;
        match self.repo.session_obj(self.id).await {
            Ok(_) => {}
            Err(Error::NotFound(_)) => return Ok(()), // finished or aborted already: nothing to abort
            Err(e) => return Err(e),
        }
        // A session whose file exists is stale (finish wrote the record, deleting the session failed).
        // Its bytes belong to the file now and must stay.
        if self.repo.file_of_upload(self.id).await?.is_none() {
            self.discard_bytes().await?;
        }
        match self.repo.objects.delete(self.id, &self.repo.actor).await {
            Ok(_) | Err(fs_objects::Error::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl UploadSession {
    /// Size of the blob if it was committed already (and nothing is pending any more).
    async fn committed_size(
        &self,
        backend: &dyn crate::backend::BlobBackend,
    ) -> Result<Option<u64>> {
        // Only a backend that names by id has a committed blob under the upload's key; on one that keeps
        // real names that key is no path (a person's file may have that name), so it is not probed.
        if backend.naming() != crate::backend::Naming::ById
            || backend.pending(&self.rec.key).await? != 0
        {
            return Ok(None);
        }
        Ok(backend.stat(&self.rec.key).await?.map(|i| i.size))
    }

    async fn discard_bytes(&self) -> Result<()> {
        self.repo
            .backends
            .get(&self.rec.backend)?
            .discard(&crate::backend::Spot::Pending(self.rec.key.clone()))
            .await?;
        Ok(())
    }
}
