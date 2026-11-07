//! `backend.list`, `backend.add`, `backend.remove`.
//!
//! Pattern: Factory + Registry — the configuration is parsed, a [`BackendFactory`](crate::BackendFactory)
//! of its kind builds the backend, `check_backend` proves it works, and only then it is swapped in
//! atomically. A failure changes nothing and never repeats the configuration (credentials).

use fs_objects::Query;

use super::consts::CONTEXT;
use super::handle::FileRepo;
use crate::backend::{BackendConfig, check_backend, overlapping};
use crate::error::{Error, Result};
use crate::model::BackendInfo;
use crate::note::{Kind, Note};

impl FileRepo {
    pub(super) async fn do_add_backend(&self, config: &str) -> Result<BackendInfo> {
        let cfg = BackendConfig::parse(config)?;
        let factory = self
            .factories
            .iter()
            .find(|f| f.kind() == cfg.kind)
            .ok_or_else(|| {
                Error::Config(
                    Note::from(format!("unknown backend kind {:?}", cfg.kind))
                        .about(Kind::Backend, &cfg.id)
                        .because("kind"),
                )
            })?;
        let _w = self.lock.lock().await;
        // Everything that can be refused is checked before anything is made.
        if self.backends.get(&cfg.id).is_ok() {
            return Err(Error::Conflict(
                Note::from(format!("a backend {} exists", cfg.id)).about(Kind::Backend, &cfg.id),
            ));
        }
        let place = factory.place(&cfg.raw).await?;
        let roots = self.backends.local_roots();
        if place
            .as_ref()
            .is_some_and(|p| overlapping(&p.path, &roots).is_some())
        {
            return Err(Error::Config(
                "the folder of a local backend is, holds, or lies in the folder of another backend"
                    .into(),
            ));
        }
        // From here on a failure takes back what this add made.
        let forget = || place.iter().for_each(|p| factory.forget(p));
        let backend = match factory.build(&cfg.raw).await {
            Ok(backend) => backend,
            Err(e) => {
                forget();
                return Err(e);
            }
        };
        if let Err(e) = check_backend(&*backend).await {
            forget();
            return Err(Error::Config(
                Note::from(format!("the backend failed its check: {e}"))
                    .about(Kind::Backend, &cfg.id)
                    .because("check"),
            ));
        }
        let added = self.backends.add(&cfg.id, backend, cfg.active);
        if added.is_err() {
            forget();
        }
        added
    }

    pub(super) async fn do_remove_backend(&self, id: &str) -> Result<()> {
        let _w = self.lock.lock().await;
        self.backends.get(id)?;
        let q = Query::new().in_context(CONTEXT).with_index("backend", id);
        let on_it = self.reads.count(&q).await?;
        if on_it > 0 {
            return Err(Error::Conflict(
                Note::from(format!(
                    "backend {id} still holds {on_it} files or uploads; move or delete them first"
                ))
                .about(Kind::Backend, id),
            ));
        }
        self.backends.remove(id)
    }
}
