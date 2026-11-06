//! `folder.rescan`: bring the records in line with a folder that people can change from outside.
//!
//! Pattern: Visitor-like walk — the disk tree and the record tree are walked together, one folder at
//! a time. Rules (provisional, owner to confirm): new files and folders are taken over (size and
//! checksum read from the bytes); a file whose size or modification time differs gets new size and
//! checksum; a file that is gone is **reported and its record kept**; nothing is ever deleted or
//! overwritten. Odd entries (symlinks, names the program refuses, a name already used by another
//! record) are listed as skipped. The whole run holds the global lock.

use std::collections::HashMap;
use std::sync::Arc;

use fs_store::{ObjectId, Query, StoredObject};

use super::build::folder_key;
use super::consts::CONTEXT;
use super::handle::FileRepo;
use super::names::check_name;
use super::steps::{Disk, Step};
use crate::backend::{BlobBackend, ScanKind, Scanned, Spot, hash_chunks};
use crate::error::{Error, Result};
use crate::model::{Entry, File, FileRecord, Folder, RescanReport};

const SNIFF: u32 = 8192;
const CHUNK_READ: u32 = 1 << 20;

/// One backend being walked.
struct Walk {
    id: String,
    backend: Arc<dyn BlobBackend>,
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
        let base = self.folder_names(folder).await?.join("/");
        let mut report = RescanReport::default();
        for (id, backend) in named {
            let walk = Walk { id, backend };
            self.walk_dir(&walk, folder, base.clone(), &mut report)
                .await?;
        }
        Ok(report)
    }

    /// All live records directly in `folder`.
    async fn children(&self, folder: Option<ObjectId>) -> Result<Vec<StoredObject>> {
        let mut out = Vec::new();
        loop {
            let mut q = Query::new()
                .in_context(CONTEXT)
                .with_index("folder", folder_key(folder))
                .with_index("trashed", "false")
                .limit(self.page);
            q.offset = out.len();
            let rows = self.reads.query(&q).await?;
            let n = rows.len();
            out.extend(rows);
            if n < self.page {
                return Ok(out);
            }
        }
    }

    fn walk_dir<'a>(
        &'a self,
        walk: &'a Walk,
        folder: Option<ObjectId>,
        path: String,
        report: &'a mut RescanReport,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let scanned = match walk.backend.scan(&path).await {
                Ok(s) => s,
                Err(Error::NotFound(_)) => Vec::new(), // the folder is gone from this disk
                Err(e) => return Err(e),
            };
            let known = self.children(folder).await?;
            let by_name: HashMap<&str, &StoredObject> = known
                .iter()
                .map(|o| (o.envelope.name.as_str(), o))
                .collect();
            let join = |name: &str| {
                if path.is_empty() {
                    name.to_string()
                } else {
                    format!("{path}/{name}")
                }
            };

            for item in &scanned {
                let at = join(&item.name);
                match (&item.kind, by_name.get(item.name.as_str())) {
                    (ScanKind::Skipped(why), _) => report.skipped.push(format!("{at}: {why}")),
                    (_, _) if check_name(&item.name).is_err() => {
                        report.skipped.push(format!("{at}: name not accepted"));
                    }
                    (ScanKind::Dir, Some(o)) if is_folder(o) => {
                        self.walk_dir(walk, Some(o.envelope.id), at, report).await?;
                    }
                    (ScanKind::Dir, None) => {
                        if self.is_legacy_shard(walk, folder, &at, &item.name).await? {
                            report.skipped.push(format!(
                                "{at}: id-keyed files of an older version, kept as they are"
                            ));
                            continue;
                        }
                        let o = self.folder_object(&item.name, folder);
                        let f = Folder::of(&o)?;
                        self.save(o).await?;
                        report.added.push(Entry::Folder(f.clone()));
                        self.walk_dir(walk, Some(f.id), at, report).await?;
                    }
                    (ScanKind::File, Some(o)) if !is_folder(o) => {
                        self.check_known(walk, o, item, &at, report).await?;
                    }
                    (ScanKind::File, None) => {
                        if is_legacy_key(&at) {
                            report
                                .skipped
                                .push(format!("{at}: id-keyed file of an older version"));
                            continue;
                        }
                        self.adopt(walk, folder, item, &at, report).await?;
                    }
                    (_, Some(_)) => report.skipped.push(format!(
                        "{at}: a {} of that name is known, but this is a {}",
                        if matches!(item.kind, ScanKind::Dir) {
                            "file"
                        } else {
                            "folder"
                        },
                        if matches!(item.kind, ScanKind::Dir) {
                            "folder"
                        } else {
                            "file"
                        },
                    )),
                }
            }
            self.report_gone(walk, &known, &scanned, &path, report)
                .await
        })
    }

    /// A file with a record: unchanged, changed, or on another backend.
    async fn check_known(
        &self,
        walk: &Walk,
        o: &StoredObject,
        item: &Scanned,
        at: &str,
        report: &mut RescanReport,
    ) -> Result<()> {
        let mut r = FileRecord::of(o)?;
        if r.backend != walk.id || !r.named {
            report.skipped.push(format!(
                "{at}: the name belongs to a record on another backend"
            ));
            return Ok(());
        }
        if r.size == item.size && r.modified == Some(item.modified) {
            return Ok(());
        }
        r.checksum = checksum_at(&*walk.backend, at, item.size).await?;
        r.size = item.size;
        r.modified = Some(item.modified);
        let mut o = o.clone();
        super::build::set_file(&mut o, &r);
        let file = File::of(&o)?;
        self.save(o).await?;
        report.updated.push(file);
        Ok(())
    }

    /// A file without a record: take it over.
    async fn adopt(
        &self,
        walk: &Walk,
        folder: Option<ObjectId>,
        item: &Scanned,
        at: &str,
        report: &mut RescanReport,
    ) -> Result<()> {
        let checksum = checksum_at(&*walk.backend, at, item.size).await?;
        let head = walk.backend.get(at, 0, SNIFF).await?;
        let media_type = infer::get(&head).map_or("application/octet-stream", |t| t.mime_type());
        let r = FileRecord {
            media_type: media_type.into(),
            size: item.size,
            checksum,
            folder,
            trashed: false,
            backend: walk.id.clone(),
            key: String::new(),
            named: true,
            modified: Some(item.modified),
        };
        let o = self.file_object(&item.name, &r);
        let file = File::of(&o)?;
        self.save(o).await?;
        report.added.push(Entry::File(file));
        Ok(())
    }

    /// Records without bytes (reported, kept) and folders without a directory (made again).
    async fn report_gone(
        &self,
        walk: &Walk,
        known: &[StoredObject],
        scanned: &[Scanned],
        path: &str,
        report: &mut RescanReport,
    ) -> Result<()> {
        for o in known {
            if scanned.iter().any(|s| s.name == o.envelope.name) {
                continue;
            }
            let at = if path.is_empty() {
                o.envelope.name.clone()
            } else {
                format!("{path}/{}", o.envelope.name)
            };
            if is_folder(o) {
                walk.backend.make_dir(&at, true).await?;
                let sub = Some(o.envelope.id);
                self.walk_dir(walk, sub, at, report).await?;
                continue;
            }
            let r = FileRecord::of(o)?;
            if r.backend != walk.id || !r.named {
                continue;
            }
            // A crash between "move to trash" and the record leaves the bytes in the trash folder: put them back.
            let back = Step::new(
                walk.backend.clone(),
                Disk::Move(Spot::Trash(o.envelope.id.to_string()), Spot::Path(at)),
                false,
            );
            if matches!(back.apply().await, Ok(true)) {
                report.updated.push(File::of(o)?);
            } else {
                report.missing.push(File::of(o)?);
            }
        }
        Ok(())
    }

    /// A top-level two-hex-digit folder that holds only id-keyed files of the first version.
    async fn is_legacy_shard(
        &self,
        walk: &Walk,
        folder: Option<ObjectId>,
        at: &str,
        name: &str,
    ) -> Result<bool> {
        let hex = name.len() == 2 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if folder.is_some() || !hex {
            return Ok(false);
        }
        let inside = walk.backend.scan(at).await?;
        Ok(!inside.is_empty()
            && inside
                .iter()
                .all(|s| s.kind == ScanKind::File && is_legacy_key(&format!("{at}/{}", s.name))))
    }
}

fn is_folder(o: &StoredObject) -> bool {
    o.envelope.object_type == super::consts::FOLDER_TYPE
}

/// `xx/<object id>` where `xx` are the last two characters of the id: a key of the first version.
fn is_legacy_key(path: &str) -> bool {
    let Some((dir, id)) = path.split_once('/') else {
        return false;
    };
    id.parse::<ObjectId>().is_ok() && id.ends_with(dir) && dir.len() == 2
}

async fn checksum_at(backend: &dyn BlobBackend, key: &str, size: u64) -> Result<String> {
    hash_chunks(size, |o, l| backend.get(key, o, l.min(CHUNK_READ))).await
}
