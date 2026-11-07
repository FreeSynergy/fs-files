//! The [`FileFeed`] contract: live changes of files and folders (`file.subscribe`).
//!
//! Pattern: Observer — a [`FileListener`] subscribes and is told about every change of a file or a
//! folder; dropping the [`Subscription`] ends it (as in `common.capnp`). [`Registry`] is the
//! reusable subject, so the trait can be implemented by anything that makes changes.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_objects::ObjectId;

/// What happened to a file or a folder. The id is all a listener gets: it asks for the entry itself
/// (`Files::get`, `Files::get_folder`), so a change that has been overtaken is not told twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileChange {
    /// A file or folder was created (an upload finished, a folder made, a rescan took one over).
    Created(ObjectId),
    /// A file or folder was changed (renamed, moved, trashed, restored, classified, rescanned).
    Updated(ObjectId),
    /// A file or folder was deleted for good (purge, folder delete). Trashing is an `Updated`.
    Deleted(ObjectId),
}

/// Receives changes. Called on the thread that made the change: keep it short, hand work off.
pub trait FileListener: Send + Sync {
    /// A file or folder changed.
    fn changed(&self, change: &FileChange);
}

/// Live updates of files and folders.
pub trait FileFeed: Send + Sync {
    /// Tell `listener` about every change until the returned [`Subscription`] is dropped.
    fn subscribe(&self, listener: Arc<dyn FileListener>) -> Subscription;
}

type Entries = Vec<(u64, Arc<dyn FileListener>)>;

/// The subject of the Observer: holds the listeners and notifies them.
#[derive(Default)]
pub struct Registry {
    next: Mutex<u64>,
    listeners: Mutex<Entries>,
}

impl Registry {
    /// Add a listener; the subscription removes it again.
    pub fn add(self: &Arc<Self>, listener: Arc<dyn FileListener>) -> Subscription {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        *next += 1;
        let id = *next;
        self.entries().push((id, listener));
        Subscription {
            id,
            registry: Arc::clone(self),
        }
    }

    /// Tell every listener.
    pub fn notify(&self, change: &FileChange) {
        let listeners: Vec<_> = self.entries().iter().map(|(_, l)| l.clone()).collect();
        for l in listeners {
            l.changed(change);
        }
    }

    fn entries(&self) -> MutexGuard<'_, Entries> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Keeps a subscription alive; dropping it unsubscribes.
#[must_use = "dropping the subscription ends it"]
pub struct Subscription {
    id: u64,
    registry: Arc<Registry>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let id = self.id;
        self.registry.entries().retain(|(i, _)| *i != id);
    }
}
