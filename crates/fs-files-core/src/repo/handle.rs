//! [`FileRepo`]: the store-backed implementation of `Files`, `Transfer` and `Backends`.
//!
//! Pattern: Repository + Facade — one small type hides the object store, the envelope rules and
//! the backends. Collaborators are injected as traits (`ObjectRepository`, `ObjectQuery`,
//! `BlobBackend`, `BackendFactory`), so each can be swapped.

use std::sync::Arc;

use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{ObjectId, Store};
use tokio::sync::Mutex;

use super::backend::StoreAdapter;
use super::consts::PAGE;
use crate::backend::{BackendFactory, BackendRegistry, BlobBackend, LocalFactory};

/// Files over an object store and a set of backends. Cheap to clone.
///
/// One `FileRepo` per store: checks like "name is free" and writes happen under one lock inside the
/// process. Two processes on the same store are not supported.
#[derive(Clone)]
pub struct FileRepo {
    pub(super) objects: Arc<dyn ObjectRepository>,
    pub(super) reads: Arc<dyn ObjectQuery>,
    pub(super) backends: Arc<BackendRegistry>,
    pub(super) factories: Arc<Vec<Arc<dyn BackendFactory>>>,
    pub(super) actor: String,
    pub(super) instance: String,
    pub(super) page: usize,
    pub(super) lock: Arc<Mutex<()>>,
    /// One lock per upload: finishing the same upload twice at once is serialised, the second gets the first's file.
    pub(super) orphan_after_secs: u64,
    pub(super) uploads: Arc<crate::backend::lock::KeyLocks>,
}

impl FileRepo {
    /// Files over a whole [`Store`]. `actor` is the IRI of the acting person / program (written to
    /// the log). `backend` (named `backend_id`) holds the bytes and is the active backend.
    pub fn new(
        store: Arc<dyn Store>,
        actor: impl Into<String>,
        backend_id: impl Into<String>,
        backend: Arc<dyn BlobBackend>,
    ) -> Self {
        let adapter = Arc::new(StoreAdapter(store));
        Self::with_backend(adapter.clone(), adapter, actor, backend_id, backend)
    }

    /// Files over the narrow store traits only (a remote or fake store can supply them).
    pub fn with_backend(
        objects: Arc<dyn ObjectRepository>,
        reads: Arc<dyn ObjectQuery>,
        actor: impl Into<String>,
        backend_id: impl Into<String>,
        backend: Arc<dyn BlobBackend>,
    ) -> Self {
        let factories: Vec<Arc<dyn BackendFactory>> = vec![
            Arc::new(LocalFactory),
            #[cfg(feature = "s3")]
            Arc::new(crate::backend::S3Factory),
        ];
        Self {
            objects,
            reads,
            backends: Arc::new(BackendRegistry::new(backend_id, backend)),
            factories: Arc::new(factories),
            actor: actor.into(),
            instance: "local.invalid".into(),
            page: PAGE,
            lock: Arc::new(Mutex::new(())),
            orphan_after_secs: 30 * 24 * 3600,
            uploads: Arc::default(),
        }
    }

    /// Instance domain for external IRIs `https://<instance>/files/<id>` (ADR 0027).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Unfinished blobs that belong to no upload are removed by `rescan` only after this long (default 30
    /// days): a quiet upload, or one of another store on the same root, must survive.
    #[must_use]
    pub fn with_orphan_after(mut self, age: std::time::Duration) -> Self {
        self.orphan_after_secs = age.as_secs();
        self
    }

    /// Entries per page (default 100, at least 1).
    #[must_use]
    pub fn with_page_size(mut self, n: usize) -> Self {
        self.page = n.max(1);
        self
    }

    /// Teach `backend.add` another kind of backend.
    #[must_use]
    pub fn with_factory(mut self, factory: Arc<dyn BackendFactory>) -> Self {
        let mut all: Vec<_> = self.factories.iter().cloned().collect();
        all.push(factory);
        self.factories = Arc::new(all);
        self
    }

    pub(super) fn iri(&self, id: ObjectId) -> String {
        format!("https://{}/files/{}", self.instance, id)
    }
}
