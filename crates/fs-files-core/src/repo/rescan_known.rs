//! The parts of `rescan` that deal with one record or one file: known, new, gone.
//!
//! Pattern: Visitor-like walk (see `rescan.rs`) — what to do when the disk and the records agree,
//! when the disk has something new, and when a record has nothing at its place.

use fs_store::{ObjectId, StoredObject};

use super::handle::FileRepo;
use super::rescan::{Item, SNIFF, Walk};
use super::steps::{Disk, Step};
use super::verify::InTrash;
use super::verify::checksum_at;
use crate::backend::{ScanKind, Scanned, Spot};
use crate::error::Result;
use crate::model::{Entry, File, FileRecord, RescanReport};

impl FileRepo {
    /// A file with a record: unchanged, changed, or not ours.
    pub(super) async fn check_known(
        &self,
        walk: &Walk,
        item: &Item,
        o: &StoredObject,
        entry: &Scanned,
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
        // A crash after "move to the trash", before the record: the recorded bytes are in the trash
        // folder, and what is at the path now is somebody else's. Finish the trash move for the old
        // record (restorable) and take the stranger's file over as a new one; never rebind.
        let in_trash = self.in_trash(&*walk.backend, o.envelope.id, &r).await?;
        if matches!(in_trash, InTrash::Stray) {
            report.skipped.push(format!(
                "{} in the trash folder is not the recorded file; left alone",
                o.envelope.id
            ));
        }
        if matches!(in_trash, InTrash::Ours) {
            r.trashed = true;
            let mut o = o.clone();
            super::build::set_file(&mut o, &r);
            let old = File::of(&o)?;
            self.save(o).await?;
            report.updated.push(old);
            return self.adopt(walk, item.folder, entry, at, report).await;
        }
        if r.size == entry.size && r.modified == Some(entry.modified) {
            return Ok(());
        }
        r.checksum = checksum_at(&*walk.backend, at, entry.size).await?;
        r.size = entry.size;
        r.modified = Some(entry.modified);
        let mut o = o.clone();
        super::build::set_file(&mut o, &r);
        let file = File::of(&o)?;
        self.save(o).await?;
        report.updated.push(file);
        Ok(())
    }

    /// A file without a record: take it over.
    pub(super) async fn adopt(
        &self,
        walk: &Walk,
        folder: Option<ObjectId>,
        entry: &Scanned,
        at: &str,
        report: &mut RescanReport,
    ) -> Result<()> {
        let checksum = checksum_at(&*walk.backend, at, entry.size).await?;
        let head = walk.backend.get(at, 0, SNIFF).await?;
        let media_type = infer::get(&head).map_or("application/octet-stream", |t| t.mime_type());
        let r = FileRecord {
            media_type: media_type.into(),
            size: entry.size,
            checksum,
            folder,
            trashed: false,
            backend: walk.id.clone(),
            key: String::new(),
            named: true,
            modified: Some(entry.modified),
            upload: None,
        };
        let o = self.file_object(&entry.name, &r);
        let file = File::of(&o)?;
        self.save(o).await?;
        report.added.push(Entry::File(file));
        Ok(())
    }

    /// A record without anything at its place: a folder is made again (and looked into, so its files
    /// are reported), a file is put back from the trash if a crash left it there, else reported missing.
    pub(super) async fn report_gone(
        &self,
        walk: &Walk,
        o: &StoredObject,
        at: &str,
        item: &Item,
        todo: &mut Vec<Item>,
        report: &mut RescanReport,
    ) -> Result<()> {
        if is_folder(o) {
            walk.backend.make_dir(at, true).await?;
            Self::enter(item, Some(o.envelope.id), at, todo, report);
            return Ok(());
        }
        let r = FileRecord::of(o)?;
        if r.backend != walk.id || !r.named {
            return Ok(());
        }
        if !matches!(
            self.in_trash(&*walk.backend, o.envelope.id, &r).await?,
            InTrash::Ours
        ) {
            report.missing.push(File::of(o)?); // nothing, or a stray file: not ours, not moved
            return Ok(());
        }
        let back = Step::new(
            walk.backend.clone(),
            Disk::Move(
                Spot::Trash(o.envelope.id.to_string()),
                Spot::Path(at.to_string()),
            ),
            false,
        );
        // Put it back only if what comes out of the trash folder is the recorded file; a stray file in
        // that spot (not ours) goes back and the record stays reported as missing.
        let restored = matches!(back.apply().await, Ok(true));
        let ours = restored
            && self
                .verify_bytes(&*walk.backend, at, &o.envelope.name, &r, true)
                .await
                .is_ok();
        if restored && !ours {
            back.undo().await;
        }
        if ours {
            report.updated.push(File::of(o)?);
        } else {
            report.missing.push(File::of(o)?);
        }
        Ok(())
    }

    /// Remove unfinished blobs that belong to no upload (a copy that crashed), if they are old enough.
    pub(super) async fn sweep_leftovers(&self, walk: &Walk) {
        if let Ok(keys) = self.session_keys(&walk.id).await {
            let _ = walk
                .backend
                .sweep_pending(&keys, self.orphan_after_secs)
                .await;
        }
    }

    /// A top-level two-hex-digit folder that holds only id-keyed files of the first version.
    pub(super) async fn is_legacy_shard(
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

pub(super) fn is_folder(o: &StoredObject) -> bool {
    o.envelope.object_type == super::consts::FOLDER_TYPE
}

/// `xx/<object id>` where `xx` are the last two characters of the id: a key of the first version.
pub(super) fn is_legacy_key(path: &str) -> bool {
    let Some((dir, id)) = path.split_once('/') else {
        return false;
    };
    id.parse::<ObjectId>().is_ok() && id.ends_with(dir) && dir.len() == 2
}
