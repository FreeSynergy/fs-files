//! Disk steps that go with a database change, and undoing them.
//!
//! Pattern: Command — each [`Step`] knows how to apply itself and how to undo itself. The rule for
//! every operation on a backend that keeps real names is: **disk first, then the database**. If a
//! disk step fails, the earlier ones are undone and nothing changed. If the database step fails,
//! all disk steps are undone. If an undo itself fails (or the process dies between the two), no byte
//! is lost: the file is where the disk says, and `rescan` takes it over and reports the old record
//! as missing.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::handle::FileRepo;
use crate::backend::{BlobBackend, Spot};
use crate::error::{Error, Result};

/// One change on a disk.
pub(super) enum Disk {
    /// Create a folder (its parents too).
    MakeDir(String),
    /// Remove an empty folder.
    RemoveDir(String),
    /// Move a file or folder; never replaces.
    Move(Spot, Spot),
    /// Move a folder (path to path). A source that is not there is fine, but then the target must not
    /// exist either: it is created empty, so nothing unknown is merged into and nothing is skipped silently.
    MoveDir(String, String),
}

/// A disk change on one backend.
pub(super) struct Step {
    pub(super) backend: Arc<dyn BlobBackend>,
    pub(super) op: Disk,
    /// A missing source is fine (the file was never on this disk).
    pub(super) optional: bool,
    /// `MoveDir` found no source and made the target instead.
    made: AtomicBool,
}

impl Step {
    pub(super) fn new(backend: Arc<dyn BlobBackend>, op: Disk, optional: bool) -> Self {
        Self {
            backend,
            op,
            optional,
            made: AtomicBool::new(false),
        }
    }

    /// Do it. `Ok(false)`: skipped because it was optional and the source is not there.
    pub(super) async fn apply(&self) -> Result<bool> {
        let done = match &self.op {
            Disk::MakeDir(path) => {
                if let Some((parent, _)) = path.rsplit_once('/') {
                    self.backend.make_dir(parent, true).await?;
                }
                self.backend.make_dir(path, false).await
            }
            Disk::RemoveDir(path) => self.backend.remove_dir(path).await,
            Disk::Move(from, to) => self.backend.relocate(from, to).await,
            Disk::MoveDir(from, to) => self.move_dir(from, to).await,
        };
        match done {
            Ok(()) => Ok(true),
            Err(Error::NotFound(_)) if self.optional => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn move_dir(&self, from: &str, to: &str) -> Result<()> {
        // A folder is expected at `from`: if it is a file now, it is somebody else's and stays.
        if matches!(self.backend.stat(from).await, Ok(Some(_))) {
            return Err(Error::Backend(format!(
                "changed outside: {from:?} is a file now, not a folder; run rescan"
            )));
        }
        let (src, dst) = (Spot::Path(from.to_string()), Spot::Path(to.to_string()));
        match self.backend.relocate(&src, &dst).await {
            Err(Error::NotFound(_)) => {
                if let Some((parent, _)) = to.rsplit_once('/') {
                    self.backend.make_dir(parent, true).await?;
                }
                self.backend.make_dir(to, false).await?; // a name clash if somebody else's folder is there
                self.made.store(true, Ordering::SeqCst);
                Ok(())
            }
            other => other,
        }
    }

    /// Put things back as well as possible. Failing here is reported by the next rescan, never silent data loss.
    pub(super) async fn undo(&self) {
        let _ = match &self.op {
            Disk::MakeDir(path) => self.backend.remove_dir(path).await,
            Disk::RemoveDir(path) => self.backend.make_dir(path, true).await,
            Disk::Move(from, to) => self.backend.relocate(to, from).await,
            Disk::MoveDir(from, to) if self.made.load(Ordering::SeqCst) => {
                self.backend.remove_dir(to).await
            }
            Disk::MoveDir(from, to) => {
                self.backend
                    .relocate(&Spot::Path(to.clone()), &Spot::Path(from.clone()))
                    .await
            }
        };
    }
}

impl FileRepo {
    /// One step per backend that keeps real names, built by `op`.
    pub(super) fn named_steps(&self, op: impl Fn() -> Disk, optional: bool) -> Vec<Step> {
        self.backends
            .named()
            .into_iter()
            .map(|(_, b)| Step::new(b, op(), optional))
            .collect()
    }

    /// Apply the steps in order, then run the database change; undo everything if either fails.
    pub(super) async fn transact<T>(
        &self,
        steps: Vec<Step>,
        db: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        let mut done = Vec::new();
        for step in steps {
            match step.apply().await {
                Ok(true) => done.push(step),
                Ok(false) => {}
                Err(e) => {
                    undo_all(&done).await;
                    return Err(e);
                }
            }
        }
        Self::then_db(&done, db).await
    }

    /// Run the database change after steps that are already applied; undo them if it fails.
    pub(super) async fn then_db<T>(
        applied: &[Step],
        db: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        match db.await {
            Ok(v) => Ok(v),
            Err(e) => {
                undo_all(applied).await;
                Err(e)
            }
        }
    }
}

async fn undo_all(done: &[Step]) {
    for step in done.iter().rev() {
        step.undo().await;
    }
}
