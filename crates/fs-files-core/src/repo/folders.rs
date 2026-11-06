//! Creating, renaming, moving and deleting folders.
//!
//! Pattern: Repository (write side) — a folder is an AS2 `Collection`. Rules: names are unique per
//! folder (case-insensitive); a folder never moves into itself or below itself; only an empty
//! folder (nothing live, nothing in the trash) can be deleted.

use fs_store::ObjectId;

use super::build::set_folder;
use super::handle::FileRepo;
use super::names::check_name;
use crate::error::{Error, Result};
use crate::model::{Folder, FolderRecord};

impl FileRepo {
    pub(super) async fn do_create_folder(
        &self,
        parent: Option<ObjectId>,
        name: &str,
    ) -> Result<Folder> {
        check_name(name)?;
        let _w = self.lock.lock().await;
        self.require_folder(parent).await?;
        self.require_free(parent, name, None).await?;
        let o = self.folder_object(name, parent);
        let folder = Folder::of(&o)?;
        self.save(o).await?;
        Ok(folder)
    }

    pub(super) async fn do_rename_folder(&self, id: ObjectId, name: &str) -> Result<Folder> {
        check_name(name)?;
        let _w = self.lock.lock().await;
        let mut o = self.folder_obj(id).await?;
        let parent = FolderRecord::of(&o)?.parent;
        self.require_free(parent, name, Some(id)).await?;
        o.envelope.name = name.to_string();
        set_folder(&mut o, parent);
        let folder = Folder::of(&o)?;
        self.save(o).await?;
        Ok(folder)
    }

    pub(super) async fn do_move_folder(
        &self,
        id: ObjectId,
        parent: Option<ObjectId>,
    ) -> Result<Folder> {
        let _w = self.lock.lock().await;
        let mut o = self.folder_obj(id).await?;
        self.require_folder(parent).await?;
        self.require_not_inside(id, parent).await?;
        self.require_free(parent, &o.envelope.name, Some(id))
            .await?;
        set_folder(&mut o, parent);
        let folder = Folder::of(&o)?;
        self.save(o).await?;
        Ok(folder)
    }

    pub(super) async fn do_delete_folder(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        self.folder_obj(id).await?;
        if self.count_in(id).await? > 0 {
            return Err(Error::Conflict(
                "the folder is not empty (files in the trash count; purge them or move them out first)".into(),
            ));
        }
        self.objects.delete(id, &self.actor).await?;
        Ok(())
    }
}
