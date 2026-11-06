//! Moving, renaming and copying files; trash, restore, purge; classification.
//!
//! Pattern: Repository (write side) with Command-like small steps — each operation is: check the
//! rules, change the record, rewrite the index, store. Trash only hides; purge deletes bytes first
//! (repeatable), metadata second.

use fs_store::ObjectId;

use super::build::set_file;
use super::handle::FileRepo;
use super::names::{check_name, copy_name};
use crate::error::{Error, Result};
use crate::model::{File, FileRecord};

impl FileRepo {
    pub(super) async fn do_move_file(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
    ) -> Result<File> {
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        self.require_folder(folder).await?;
        self.require_free(folder, &o.envelope.name, Some(id))
            .await?;
        let mut r = FileRecord::of(&o)?;
        r.folder = folder;
        self.store_file(&mut o, &r).await
    }

    pub(super) async fn do_rename_file(&self, id: ObjectId, name: &str) -> Result<File> {
        check_name(name)?;
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        let r = FileRecord::of(&o)?;
        self.require_free(r.folder, name, Some(id)).await?;
        o.envelope.name = name.to_string();
        self.store_file(&mut o, &r).await
    }

    pub(super) async fn do_trash(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        let mut o = self.live_file_obj(id).await?;
        let mut r = FileRecord::of(&o)?;
        r.trashed = true;
        self.store_file(&mut o, &r).await.map(|_| ())
    }

    pub(super) async fn do_restore(&self, id: ObjectId) -> Result<File> {
        let _w = self.lock.lock().await;
        let mut o = self.file_obj(id).await?;
        let mut r = FileRecord::of(&o)?;
        if !r.trashed {
            return Err(Error::Conflict(format!("file {id} is not in the trash")));
        }
        self.require_folder(r.folder).await?;
        self.require_free(r.folder, &o.envelope.name, Some(id))
            .await?;
        r.trashed = false;
        self.store_file(&mut o, &r).await
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
        self.backends.get(&r.backend)?.delete(&r.key).await?;
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

    pub(super) async fn do_copy_file(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
    ) -> Result<File> {
        let src = self.live_file_obj(id).await?;
        self.require_folder(folder).await?;
        let mut r = FileRecord::of(&src)?;
        let key = self.copy_bytes(&r).await?;
        r.key = key;
        r.folder = folder;
        let _w = self.lock.lock().await;
        let name = self.free_copy_name(folder, &src.envelope.name).await?;
        let mut o = self.file_object(&name, &r);
        o.envelope.sensitive.clone_from(&src.envelope.sensitive);
        let file = File::of(&o)?;
        if let Err(e) = self.save(o).await {
            self.backends.get(&r.backend)?.delete(&r.key).await?;
            return Err(e);
        }
        Ok(file)
    }

    async fn free_copy_name(&self, folder: Option<ObjectId>, name: &str) -> Result<String> {
        if self.require_free(folder, name, None).await.is_ok() {
            return Ok(name.to_string());
        }
        for n in 1..1000 {
            let candidate = copy_name(name, n);
            if self.require_free(folder, &candidate, None).await.is_ok() {
                return Ok(candidate);
            }
        }
        Err(Error::Conflict("no free name for the copy".into()))
    }

    async fn store_file(&self, o: &mut fs_store::StoredObject, r: &FileRecord) -> Result<File> {
        set_file(o, r);
        let file = File::of(o)?;
        self.save(o.clone()).await?;
        Ok(file)
    }
}
