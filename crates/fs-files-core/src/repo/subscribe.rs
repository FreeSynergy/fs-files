//! `FileFeed` of the store-backed repository.
//!
//! Pattern: Observer (subject) — the repository owns a [`Registry`]; `Notifying` (see `notify.rs`)
//! feeds it, this only lets listeners in.

use std::sync::Arc;

use super::handle::FileRepo;
use crate::feed::{FileFeed, FileListener, Subscription};

impl FileFeed for FileRepo {
    fn subscribe(&self, listener: Arc<dyn FileListener>) -> Subscription {
        self.feed.add(listener)
    }
}
