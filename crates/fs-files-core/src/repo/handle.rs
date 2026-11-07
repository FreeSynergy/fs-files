//! [`FileRepo`]: the store-backed implementation of `Files`, `Transfer` and `Backends`.
//!
//! Pattern: Repository + Facade — one small type hides the object store, the envelope rules and
//! the backends. Collaborators are injected as traits (`ObjectRepository`, `ObjectQuery`,
//! `BlobBackend`, `BackendFactory`), so each can be swapped.

use std::sync::Arc;

use fs_objects::traits::{ObjectQuery, ObjectRepository};
use fs_objects::{ObjectId, Store, StoreInfo};
use tokio::sync::Mutex;

use super::backend::StoreAdapter;
use super::consts::PAGE;
use super::notify::Notifying;
use crate::backend::{BackendFactory, BackendRegistry, BlobBackend, LocalFactory};
use crate::feed::Registry;

/// The shortest age at which `rescan` removes an unfinished blob without an upload (one hour).
const MIN_ORPHAN_AFTER_SECS: u64 = 3600;

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
    pub(super) info: Option<Arc<dyn StoreInfo>>,
    pub(super) page: usize,
    pub(super) lock: Arc<Mutex<()>>,
    /// One lock per upload: finishing the same upload twice at once is serialised, the second gets the first's file.
    pub(super) orphan_after_secs: u64,
    pub(super) uploads: Arc<crate::backend::lock::KeyLocks>,
    /// The subject of `FileFeed`: told by `objects`, which wraps every write.
    pub(super) feed: Arc<Registry>,
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
        Self::with_backend(adapter.clone(), adapter.clone(), actor, backend_id, backend)
            .with_info(adapter)
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
        let feed = Arc::new(Registry::default());
        let objects = Arc::new(Notifying {
            inner: objects,
            feed: feed.clone(),
        });
        Self {
            objects,
            reads,
            backends: Arc::new(BackendRegistry::new(backend_id, backend)),
            factories: Arc::new(factories),
            actor: actor.into(),
            instance: "local.invalid".into(),
            info: None,
            page: PAGE,
            lock: Arc::new(Mutex::new(())),
            orphan_after_secs: 30 * 24 * 3600,
            uploads: Arc::default(),
            feed,
        }
    }

    /// Say whose store this is: [`Files::identity`](crate::Files::identity) answers from it.
    #[must_use]
    pub fn with_info(mut self, info: Arc<dyn StoreInfo>) -> Self {
        self.info = Some(info);
        self
    }

    /// Instance domain for external IRIs `https://<instance>/files/<id>` (ADR 0027).
    #[must_use]
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Unfinished blobs that belong to no upload are removed by `rescan` only after this long (default 30
    /// days, never less than an hour): a quiet upload must survive. An upload of another store on the same root that is older than the limit is removed too: stores that share a root must share the limit.
    #[must_use]
    pub fn with_orphan_after(mut self, age: std::time::Duration) -> Self {
        self.orphan_after_secs = age.as_secs().max(MIN_ORPHAN_AFTER_SECS);
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

    /// Let `backend.add` make local backends only below `base`, with a path relative to it (see
    /// `ConfinedLocalFactory`). Replaces the factory that takes any path. A program whose
    /// `backend.add` is open to other programs should always set this.
    #[must_use]
    pub fn with_local_base(mut self, base: impl AsRef<std::path::Path>) -> Self {
        let mut all: Vec<_> = self
            .factories
            .iter()
            .filter(|f| f.kind() != "local")
            .cloned()
            .collect();
        all.push(Arc::new(crate::backend::ConfinedLocalFactory::new(
            base.as_ref(),
        )));
        self.factories = Arc::new(all);
        self
    }

    pub(super) fn iri(&self, id: ObjectId) -> String {
        format!("https://{}/files/{}", self.instance, id)
    }
}
