//! `backend.list`, `backend.add`, `backend.remove`.
//!
//! Pattern: Factory + Registry — the configuration is parsed, a [`BackendFactory`](crate::BackendFactory)
//! of its kind builds the backend, `check_backend` proves it works, and only then it is swapped in
//! atomically. A failure changes nothing and never repeats the configuration (credentials).

use fs_store::Query;

use super::consts::CONTEXT;
use super::handle::FileRepo;
use crate::backend::{BackendConfig, check_backend};
use crate::error::{Error, Result};
use crate::model::BackendInfo;

impl FileRepo {
    pub(super) async fn do_add_backend(&self, config: &str) -> Result<BackendInfo> {
        let cfg = BackendConfig::parse(config)?;
        let factory = self
            .factories
            .iter()
            .find(|f| f.kind() == cfg.kind)
            .ok_or_else(|| Error::Config(format!("unknown backend kind {:?}", cfg.kind)))?;
        let backend = factory.build(&cfg.raw).await?;
        check_backend(&*backend)
            .await
            .map_err(|e| Error::Config(format!("the backend failed its check: {e}")))?;
        self.backends.add(&cfg.id, backend, cfg.active)
    }

    pub(super) async fn do_remove_backend(&self, id: &str) -> Result<()> {
        let _w = self.lock.lock().await;
        self.backends.get(id)?;
        let q = Query::new().in_context(CONTEXT).with_index("backend", id);
        let on_it = self.reads.count(&q).await?;
        if on_it > 0 {
            return Err(Error::Conflict(format!(
                "backend {id} still holds {on_it} files or uploads; move or delete them first"
            )));
        }
        self.backends.remove(id)
    }
}
