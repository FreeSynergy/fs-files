//! Upload sessions: start, resume, write, ask how much arrived, abort.
//!
//! Pattern: State — a session is an object in the store (`Upload`, written once at the start) plus
//! the pending bytes in the backend. The position is *not* stored separately: it is what the backend
//! holds (`BlobBackend::pending`), so it cannot disagree with the bytes and survives any restart.

use std::sync::Arc;

use async_trait::async_trait;
use fs_store::{ObjectId, StoredObject};
use serde::{Deserialize, Serialize};

use super::blob::new_key;
use super::handle::FileRepo;
use super::names::check_name;
use crate::error::{Error, Result};
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
    ) -> Result<(Arc<dyn Upload>, String)> {
        check_name(name)?;
        self.require_folder(folder).await?;
        self.require_free(folder, name, None).await?;
        let (backend_id, backend) = self.backends.active();
        let probe = ObjectId::new();
        let rec = SessionRecord {
            folder,
            name: name.to_string(),
            media_type: media_type.trim().to_ascii_lowercase(),
            backend: backend_id,
            key: new_key(probe),
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

    pub(super) async fn do_resume_upload(&self, id: &str) -> Result<Arc<dyn Upload>> {
        let id: ObjectId = id
            .parse()
            .map_err(|_| Error::NotFound(format!("upload {id}")))?;
        let o = self.session_obj(id).await?;
        let rec = serde_json::from_value(o.content)?;
        Ok(Arc::new(UploadSession {
            repo: self.clone(),
            id,
            rec,
        }))
    }

    pub(super) async fn session_obj(&self, id: ObjectId) -> Result<StoredObject> {
        match self.objects.get(id).await? {
            Some(o) if o.index.get("kind").is_some_and(|k| k == &["upload"]) => Ok(o),
            _ => Err(Error::NotFound(format!("upload {id}"))),
        }
    }
}

#[async_trait]
impl Upload for UploadSession {
    async fn write(&self, offset: u64, data: &[u8]) -> Result<()> {
        self.repo.session_obj(self.id).await?; // finished or aborted sessions take nothing
        let backend = self.repo.backends.get(&self.rec.backend)?;
        backend.put(&self.rec.key, offset, data).await
    }

    async fn received(&self) -> Result<u64> {
        self.repo
            .backends
            .get(&self.rec.backend)?
            .pending(&self.rec.key)
            .await
    }

    async fn finish(&self) -> Result<crate::model::File> {
        self.repo.finish_upload(self.id, &self.rec).await
    }

    async fn abort(&self) -> Result<()> {
        self.repo
            .backends
            .get(&self.rec.backend)?
            .delete(&self.rec.key)
            .await?;
        match self.repo.objects.delete(self.id, &self.repo.actor).await {
            Ok(_) | Err(fs_store::Error::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
