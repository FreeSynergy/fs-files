//! One async lock per key.
//!
//! Pattern: RAII / Guard — `KeyLocks::lock` returns a guard; two writers of the same key are
//! serialised, different keys never wait for each other.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// A set of locks addressed by key.
#[derive(Default)]
pub(crate) struct KeyLocks(Mutex<HashMap<String, Arc<AsyncMutex<()>>>>);

impl KeyLocks {
    /// Wait for the lock of `key`.
    pub(crate) async fn lock(&self, key: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut map = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.retain(|_, l| Arc::strong_count(l) > 1); // forget locks nobody holds
            map.entry(key.to_string()).or_default().clone()
        };
        lock.lock_owned().await
    }
}
