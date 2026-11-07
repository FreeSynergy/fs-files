//! The backends currently in use, swapped atomically.
//!
//! Pattern: Registry + Memento (snapshots) — readers take an `Arc` snapshot and see either the old
//! or the new set, never a mix; `add` and `remove` build a new snapshot and replace it in one step.

use std::sync::{Arc, RwLock};

use super::BlobBackend;
use crate::error::{Error, Result};
use crate::model::BackendInfo;

/// An immutable set of backends. `active` takes new files.
#[derive(Clone)]
struct Snapshot {
    entries: Vec<(String, Arc<dyn BlobBackend>)>,
    /// Index into `entries`.
    active: usize,
}

/// The swappable set of backends of a repository.
pub struct BackendRegistry {
    current: RwLock<Arc<Snapshot>>,
}

impl BackendRegistry {
    /// A registry with one backend, which is the active one.
    pub fn new(id: impl Into<String>, backend: Arc<dyn BlobBackend>) -> Self {
        let snap = Snapshot {
            entries: vec![(id.into(), backend)],
            active: 0,
        };
        Self {
            current: RwLock::new(Arc::new(snap)),
        }
    }

    fn snapshot(&self) -> Arc<Snapshot> {
        self.current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn swap(&self, next: Snapshot) {
        *self
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::new(next);
    }

    /// The backend with this id.
    pub fn get(&self, id: &str) -> Result<Arc<dyn BlobBackend>> {
        let snap = self.snapshot();
        let found = snap.entries.iter().find(|(i, _)| i == id);
        found
            .map(|(_, b)| b.clone())
            .ok_or_else(|| Error::NotFound(format!("backend {id}")))
    }

    /// Every backend that keeps real names, with its id (their folders must follow ours).
    pub fn named(&self) -> Vec<(String, Arc<dyn BlobBackend>)> {
        let snap = self.snapshot();
        let by_name = |e: &&(String, Arc<dyn BlobBackend>)| e.1.naming() == super::Naming::ByName;
        snap.entries.iter().filter(by_name).cloned().collect()
    }

    /// The folders of the backends that live on this machine.
    pub fn local_roots(&self) -> Vec<std::path::PathBuf> {
        let snap = self.snapshot();
        let root = |e: &(String, Arc<dyn BlobBackend>)| e.1.local_root().map(Into::into);
        snap.entries.iter().filter_map(root).collect()
    }

    /// Id and backend that take new files.
    pub fn active(&self) -> (String, Arc<dyn BlobBackend>) {
        let snap = self.snapshot();
        let (id, backend) = &snap.entries[snap.active];
        (id.clone(), backend.clone())
    }

    /// What `backend.list` reports.
    pub fn list(&self) -> Vec<BackendInfo> {
        let snap = self.snapshot();
        let info = |(n, (id, b)): (usize, &(String, Arc<dyn BlobBackend>))| BackendInfo {
            id: id.clone(),
            kind: b.kind().to_string(),
            active: n == snap.active,
        };
        snap.entries.iter().enumerate().map(info).collect()
    }

    /// Add a (checked) backend; with `activate` it takes new files from now on.
    pub fn add(
        &self,
        id: &str,
        backend: Arc<dyn BlobBackend>,
        activate: bool,
    ) -> Result<BackendInfo> {
        let mut next = (*self.snapshot()).clone();
        if next.entries.iter().any(|(i, _)| i == id) {
            return Err(Error::Conflict(format!("a backend {id} exists")));
        }
        let kind = backend.kind().to_string();
        next.entries.push((id.to_string(), backend));
        if activate {
            next.active = next.entries.len() - 1;
        }
        let active = next.active == next.entries.len() - 1;
        self.swap(next);
        Ok(BackendInfo {
            id: id.to_string(),
            kind,
            active,
        })
    }

    /// Take a backend out. The active one stays: add another as active first.
    pub fn remove(&self, id: &str) -> Result<()> {
        let mut next = (*self.snapshot()).clone();
        let at = next
            .entries
            .iter()
            .position(|(i, _)| i == id)
            .ok_or_else(|| Error::NotFound(format!("backend {id}")))?;
        if at == next.active {
            return Err(Error::Conflict(format!(
                "backend {id} is the active one; activate another first"
            )));
        }
        next.entries.remove(at);
        if at < next.active {
            next.active -= 1;
        }
        self.swap(next);
        Ok(())
    }
}
