//! Changes of the provider to a subscribed consumer.
//!
//! Pattern: Observer + Adapter — the core calls its listener on the thread that made the change; this
//! one only puts the change into a channel. The shared forwarder adds the envelope and sends it.

use std::sync::Arc;

use fs_files_core::{FileChange, FileFeed, FileListener, Files};
use fs_interfaces::Unsupported;
use fs_interfaces::common_capnp::{change_listener, subscription};
use fs_interfaces::rpc::{ChangeSender, Notice, change_channel, forward, unsupported};
use fs_store::{Envelope, ObjectId};

struct ToChannel(ChangeSender<FileChange>);

impl FileListener for ToChannel {
    fn changed(&self, change: &FileChange) {
        self.0.send(*change);
    }
}

const SUBSCRIBE: &str = "file.subscribe";

/// A provider without a feed says so in its report; the capability would otherwise stay silent.
pub(crate) fn offered(files: &dyn Files) -> capnp::Result<()> {
    let report = files.capabilities();
    if report.is_supported(SUBSCRIBE) {
        return Ok(());
    }
    let why = report
        .why_not(SUBSCRIBE)
        .unwrap_or_else(|| Unsupported::new(SUBSCRIBE, "this provider does not offer it"));
    Err(unsupported(&why))
}

/// Subscribe `listener` until the returned capability is dropped.
pub(crate) fn subscribe(
    feed: &Arc<dyn FileFeed>,
    files: Arc<dyn Files>,
    listener: change_listener::Client,
) -> subscription::Client {
    let (tx, rx) = change_channel();
    let core_subscription = feed.subscribe(Arc::new(ToChannel(tx)));
    forward(rx, core_subscription, listener, move |change| {
        let files = files.clone();
        async move { notice(files.as_ref(), change).await }
    })
}

/// The envelope of a file or a folder, if there is one by now.
async fn envelope_of(files: &dyn Files, id: ObjectId) -> Result<Option<Envelope>, String> {
    if let Some(file) = files.get(id).await.map_err(|e| e.to_string())? {
        return Ok(Some(file.envelope));
    }
    let folder = files.get_folder(id).await.map_err(|e| e.to_string())?;
    Ok(folder.map(|f| f.envelope))
}

/// An entry that is gone again by now has nothing to report but its own `Deleted`, which follows. A
/// read that fails is no such thing: the change cannot be told, and that ends the feed.
async fn notice(files: &dyn Files, change: FileChange) -> Result<Option<Notice>, String> {
    Ok(match change {
        FileChange::Created(id) => envelope_of(files, id).await?.map(Notice::Created),
        FileChange::Updated(id) => envelope_of(files, id).await?.map(Notice::Updated),
        FileChange::Deleted(id) => Some(Notice::Deleted(id)),
    })
}
