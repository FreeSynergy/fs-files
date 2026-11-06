//! `folder.rescan`: bring the records in line with a folder that people can change from outside.
//!
//! Pattern: Visitor-like walk — the disk tree and the record tree are walked together, one folder at
//! a time, with a work list instead of recursion (a deep tree cannot overflow the stack). Rules
//! (provisional, owner to confirm): new files and folders are taken over (size and checksum read from
//! the bytes); a file whose size or modification time differs gets new size and checksum; a file that
//! is gone is **reported and its record kept**; nothing is ever deleted or overwritten. Entries that
//! cannot be handled (symlinks, odd names, unreadable files, paths that are too long, a name used by
//! another record) are listed as skipped with the reason, and the walk goes on. The whole run holds
//! the global lock.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fs_store::{ObjectId, Query, StoredObject};

use super::build::folder_key;
use super::consts::CONTEXT;
use super::handle::FileRepo;
use super::names::check_name;
use super::rescan_known::{is_folder, is_legacy_key};
use crate::backend::{BlobBackend, ScanKind, Scanned};
use crate::error::{Error, Result};
use crate::model::{Entry, Folder, RescanReport};

pub(super) const SNIFF: u32 = 8192;
/// Folders nested deeper than this are not entered (reported as skipped).
const MAX_DEPTH: usize = 512;
/// Records are read this many at a time.
const BATCH: usize = 1000;

/// One backend being walked.
pub(super) struct Walk {
    pub(super) id: String,
    pub(super) backend: Arc<dyn BlobBackend>,
}

/// A folder still to look at.
pub(super) struct Item {
    pub(super) folder: Option<ObjectId>,
    pub(super) path: String,
    pub(super) depth: usize,
}

pub(super) fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}/{name}")
    }
}

impl FileRepo {
    pub(super) async fn do_rescan(&self, folder: Option<ObjectId>) -> Result<RescanReport> {
        let _w = self.lock.lock().await;
        self.require_folder(folder).await?;
        let named = self.backends.named();
        if named.is_empty() {
            return Err(fs_interfaces::Unsupported::new(
                "folder.rescan",
                "no backend keeps files under their real names",
            )
            .into());
        }
        let parents = self.folder_names(folder).await?;
        let mut report = RescanReport::default();
        for (id, backend) in named {
            let walk = Walk { id, backend };
            let tree = self.load_tree().await?;
            let mut todo = vec![Item {
                folder,
                path: parents.join("/"),
                depth: parents.len(),
            }];
            while let Some(item) = todo.pop() {
                self.visit_dir(&walk, &tree, &item, &mut todo, &mut report)
                    .await?;
            }
            if folder.is_none() {
                self.sweep_leftovers(&walk).await;
            }
        }
        Ok(report)
    }

    /// Every live file and folder record, grouped by the folder they are in, read with one query.
    /// (The store's filtered queries cost about a second per condition per thousand records, so the
    /// walk reads everything once and filters here.)
    async fn load_tree(&self) -> Result<HashMap<String, Vec<StoredObject>>> {
        let mut tree: HashMap<String, Vec<StoredObject>> = HashMap::new();
        let mut seen = 0;
        loop {
            let mut q = Query::new().in_context(CONTEXT).limit(BATCH);
            q.offset = seen;
            let rows = self.reads.query(&q).await?;
            seen += rows.len();
            let n = rows.len();
            for o in rows {
                let one = |k: &str| o.index.get(k).and_then(|v| v.first()).cloned();
                let live = one("trashed").as_deref() == Some("false");
                if let (true, Some(folder)) = (live, one("folder")) {
                    tree.entry(folder).or_default().push(o);
                }
            }
            if n < BATCH {
                return Ok(tree);
            }
        }
    }

    /// Look at one folder: its entries on the disk against its records. Folders found go on `todo`.
    async fn visit_dir(
        &self,
        walk: &Walk,
        tree: &HashMap<String, Vec<StoredObject>>,
        item: &Item,
        todo: &mut Vec<Item>,
        report: &mut RescanReport,
    ) -> Result<()> {
        let scanned = match walk.backend.scan(&item.path).await {
            Ok(s) => s,
            Err(Error::NotFound(_)) => Vec::new(), // the folder is gone from this disk
            Err(e) => {
                report.skipped.push(format!("{}: {e}", item.path));
                return Ok(());
            }
        };
        let known: &[StoredObject] = tree
            .get(&folder_key(item.folder))
            .map_or(&[], Vec::as_slice);
        let by_name: HashMap<&str, &StoredObject> = known
            .iter()
            .map(|o| (o.envelope.name.as_str(), o))
            .collect();
        for entry in &scanned {
            let at = join(&item.path, &entry.name);
            let seen = by_name.get(entry.name.as_str()).copied();
            if let Err(e) = self
                .visit_entry(walk, item, entry, seen, &at, todo, report)
                .await
            {
                report.skipped.push(format!("{at}: {e}"));
            }
        }
        let on_disk: HashSet<&str> = scanned.iter().map(|s| s.name.as_str()).collect();
        for o in known
            .iter()
            .filter(|o| !on_disk.contains(o.envelope.name.as_str()))
        {
            let at = join(&item.path, &o.envelope.name);
            if let Err(e) = self.report_gone(walk, o, &at, item, todo, report).await {
                report.skipped.push(format!("{at}: {e}"));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // one entry of the walk and everything it needs
    async fn visit_entry(
        &self,
        walk: &Walk,
        item: &Item,
        entry: &Scanned,
        seen: Option<&StoredObject>,
        at: &str,
        todo: &mut Vec<Item>,
        report: &mut RescanReport,
    ) -> Result<()> {
        if let ScanKind::Skipped(why) = &entry.kind {
            report.skipped.push(format!("{at}: {why}"));
            return Ok(());
        }
        check_name(&entry.name)?;
        let dir = entry.kind == ScanKind::Dir;
        match (dir, seen.map(is_folder)) {
            (true, Some(true)) => Self::enter(item, seen.map(|o| o.envelope.id), at, todo, report),
            (true, None) => {
                if self
                    .is_legacy_shard(walk, item.folder, at, &entry.name)
                    .await?
                {
                    report.skipped.push(format!(
                        "{at}: id-keyed files of an older version, kept as they are"
                    ));
                    return Ok(());
                }
                let o = self.folder_object(&entry.name, item.folder);
                let f = Folder::of(&o)?;
                self.save(o).await?;
                report.added.push(Entry::Folder(f.clone()));
                Self::enter(item, Some(f.id), at, todo, report);
            }
            (false, Some(false)) => {
                let o = seen.ok_or_else(|| Error::NotFound(at.into()))?;
                self.check_known(walk, item, o, entry, at, report).await?;
            }
            (false, None) => {
                if is_legacy_key(at) {
                    report
                        .skipped
                        .push(format!("{at}: id-keyed file of an older version"));
                } else {
                    self.adopt(walk, item.folder, entry, at, report).await?;
                }
            }
            (_, Some(_)) => report.skipped.push(format!(
                "{at}: a {} of that name is known, but this is a {}",
                if dir { "file" } else { "folder" },
                if dir { "folder" } else { "file" },
            )),
        }
        Ok(())
    }

    pub(super) fn enter(
        parent: &Item,
        folder: Option<ObjectId>,
        at: &str,
        todo: &mut Vec<Item>,
        report: &mut RescanReport,
    ) {
        if parent.depth + 1 > MAX_DEPTH {
            report.skipped.push(format!("{at}: nested too deep"));
            return;
        }
        todo.push(Item {
            folder,
            path: at.to_string(),
            depth: parent.depth + 1,
        });
    }
}
