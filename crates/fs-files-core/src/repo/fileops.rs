//! Moving, renaming and copying files; trash, restore, purge; classification.
//!
//! Pattern: Repository (write side) with Command-like small steps — each operation is: check the
//! rules, do the disk step if the file lives under its real name (`steps.rs`: disk first, undone if
//! the database fails), change the record, rewrite the index, store. Trash moves the bytes to the
//! program's own trash folder; purge deletes bytes first (repeatable), metadata second.

use fs_store::ObjectId;

use super::build::set_file;
use super::handle::FileRepo;
use super::names::check_name;
use super::steps::{Disk, Step};
use crate::backend::Spot;
use crate::error::{Error, Result};
use crate::model::{File, FileRecord, OnConflict, TrashReport};
use fs_store::{Query, StoredObject};

use super::consts::CONTEXT;

/// Where a restored file goes.
pub(super) enum Target {
    /// Where it was.
    Original,
    /// This folder (`None` = top level).
    Folder(Option<ObjectId>),
}

impl FileRepo {
    pub(super) async fn do_move_file(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        self.require_folder(folder).await?;
        let name = self
            .resolve_name(folder, &o.envelope.name, Some(id), on, false)
            .await?;
        let mut r = FileRecord::of(&o)?;
        let steps = self.rename_steps(&o, &r, folder, &name).await?;
        o.envelope.name = name;
        r.folder = folder;
        self.store_with(steps, &mut o, &r).await
    }

    pub(super) async fn do_rename_file(
        &self,
        id: ObjectId,
        name: &str,
        on: OnConflict,
    ) -> Result<File> {
        check_name(name)?;
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        let name = self
            .resolve_name(r.folder, name, Some(id), on, false)
            .await?;
        let steps = self.rename_steps(&o, &r, r.folder, &name).await?;
        o.envelope.name = name;
        self.store_with(steps, &mut o, &r).await
    }

    pub(super) async fn do_trash(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        let mut r = FileRecord::of(&o)?;
        let mut steps = Vec::new();
        if r.named {
            let from = Spot::Path(self.key_of(&o, &r).await?);
            let to = Spot::Trash(id.to_string());
            steps.push(Step::new(
                self.backends.get(&r.backend)?,
                Disk::Move(from, to),
                true,
            ));
        }
        r.trashed = true;
        self.store_with(steps, &mut o, &r).await.map(|_| ())
    }

    /// Restore to the original folder or to another one.
    pub(super) async fn do_restore(
        &self,
        id: ObjectId,
        target: Target,
        on: OnConflict,
    ) -> Result<File> {
        let _w = self.lock.lock().await;
        let mut o = self.file_obj(id).await?;
        let mut r = FileRecord::of(&o)?;
        if !r.trashed {
            return Err(Error::Conflict(format!("file {id} is not in the trash")));
        }
        if let Target::Folder(f) = target {
            r.folder = f;
        }
        self.require_folder(r.folder).await?;
        o.envelope.name = self
            .resolve_name(r.folder, &o.envelope.name, Some(id), on, false)
            .await?;
        if !r.named {
            r.trashed = false;
            return self.store_file(&mut o, &r).await;
        }
        let backend = self.backends.get(&r.backend)?;
        let key = self.path_in(r.folder, &o.envelope.name).await?;
        let step = Step::new(
            backend.clone(),
            Disk::Move(Spot::Trash(id.to_string()), Spot::Path(key.clone())),
            false,
        );
        r.trashed = false;
        set_file(&mut o, &r);
        let file = File::of(&o)?;
        match step.apply().await {
            Ok(_) => Self::then_db(&[step], self.save(o)).await?,
            // The bytes are not in the trash. After a crash between the two steps of an earlier
            // restore they are already at their place: then only the record is missing.
            Err(Error::NotFound(_))
                if backend.stat(&key).await?.is_some_and(|i| i.size == r.size) =>
            {
                self.save(o).await?;
            }
            Err(Error::NotFound(_)) => {
                return Err(Error::NotFound(format!(
                    "changed outside: the bytes of file {id} are neither in the trash nor at {key:?}; run rescan"
                )));
            }
            Err(e) => return Err(e),
        }
        Ok(file)
    }

    pub(super) async fn do_purge(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        let o = self.file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        if !r.trashed {
            return Err(Error::Conflict(
                "only a file in the trash can be purged; trash it first".into(),
            ));
        }
        // Bytes first: deleting is repeatable, so a failure leaves a file that can be purged again.
        let backend = self.backends.get(&r.backend)?;
        if r.named {
            backend.discard(&Spot::Trash(id.to_string())).await?;
        } else {
            backend.delete(&r.key).await?;
        }
        self.objects.delete(id, &self.actor).await?;
        Ok(())
    }

    pub(super) async fn do_classify(&self, id: ObjectId, categories: Vec<String>) -> Result<File> {
        let _w = self.lock.lock().await;
        let mut o = self.file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        let mut categories = categories;
        categories.sort();
        categories.dedup();
        o.envelope.sensitive = categories; // ADR 0029: the store decides whether it may hold them
        self.store_file(&mut o, &r).await
    }

    /// The disk step of a rename or a move of a file that lives under its real name.
    async fn rename_steps(
        &self,
        o: &StoredObject,
        r: &FileRecord,
        folder: Option<ObjectId>,
        name: &str,
    ) -> Result<Vec<Step>> {
        if !r.named {
            return Ok(Vec::new());
        }
        let from = self.key_of(o, r).await?;
        let to = self.path_in(folder, name).await?;
        if from == to {
            return Ok(Vec::new());
        }
        let backend = self.backends.get(&r.backend)?;
        Ok(vec![Step::new(
            backend,
            Disk::Move(Spot::Path(from), Spot::Path(to)),
            false,
        )])
    }

    async fn store_with(
        &self,
        steps: Vec<Step>,
        o: &mut StoredObject,
        r: &FileRecord,
    ) -> Result<File> {
        set_file(o, r);
        let file = File::of(o)?;
        self.transact(steps, self.save(o.clone())).await?;
        Ok(file)
    }

    async fn store_file(&self, o: &mut fs_store::StoredObject, r: &FileRecord) -> Result<File> {
        set_file(o, r);
        let file = File::of(o)?;
        self.save(o.clone()).await?;
        Ok(file)
    }
}

impl FileRepo {
    /// Purge every file in the trash. A file that cannot be purged is reported, the others are still purged.
    pub(super) async fn do_empty_trash(&self) -> Result<TrashReport> {
        let mut ids = Vec::new();
        loop {
            let mut q = Query::new()
                .in_context(CONTEXT)
                .with_index("kind", "file")
                .with_index("trashed", "true")
                .limit(self.page);
            q.offset = ids.len();
            let rows = self.reads.query(&q).await?;
            let n = rows.len();
            ids.extend(rows.iter().map(|o| o.envelope.id));
            if n < self.page {
                break;
            }
        }
        let mut report = TrashReport::default();
        for id in ids {
            match self.do_purge(id).await {
                Ok(()) => report.purged += 1,
                Err(_) => report.failed.push(id),
            }
        }
        Ok(report)
    }
}
