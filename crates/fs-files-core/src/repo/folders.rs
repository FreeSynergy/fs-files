//! Creating, renaming, moving and deleting folders.
//!
//! Pattern: Repository (write side) — a folder is an AS2 `Collection`. Rules: names are unique per
//! folder (exact); a folder never moves into itself or below itself; only an empty
//! folder (nothing live, nothing in the trash) can be deleted.

use fs_objects::ObjectId;

use super::build::set_folder;
use super::handle::FileRepo;
use super::names::check_name;
use super::steps::Disk;
use crate::error::{Error, Result};
use crate::model::OnConflict;
use crate::model::{Folder, FolderRecord};
use crate::note::{Kind, Note};

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
        let path = self.path_in(parent, name).await?;
        let steps = self.named_steps(|| Disk::MakeDir(path.clone()), false);
        let o = self.folder_object(name, parent);
        self.transact(steps, self.save_folder(o)).await
    }

    pub(super) async fn do_rename_folder(
        &self,
        id: ObjectId,
        name: &str,
        on: OnConflict,
    ) -> Result<Folder> {
        check_name(name)?;
        let _w = self.lock.lock().await;
        let mut o = self.folder_obj(id).await?;
        let parent = FolderRecord::of(&o)?.parent;
        let old = o.envelope.name.clone();
        let name = self
            .resolve_name(parent, name, Some(id), on, false, Some(&old))
            .await?;
        self.check_folder_fits(id, parent, &name).await?;
        let steps = self
            .move_steps(parent, &o.envelope.name, parent, &name)
            .await?;
        o.envelope.name = name;
        set_folder(&mut o, parent);
        self.transact(steps, self.save_folder(o)).await
    }

    pub(super) async fn do_move_folder(
        &self,
        id: ObjectId,
        parent: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<Folder> {
        let _w = self.lock.lock().await;
        let mut o = self.folder_obj(id).await?;
        self.require_folder(parent).await?;
        self.require_not_inside(id, parent).await?;
        let same = (FolderRecord::of(&o)?.parent == parent).then_some(o.envelope.name.as_str());
        let name = self
            .resolve_name(parent, &o.envelope.name, Some(id), on, false, same)
            .await?;
        self.check_folder_fits(id, parent, &name).await?;
        let from = FolderRecord::of(&o)?.parent;
        let steps = self
            .move_steps(from, &o.envelope.name, parent, &name)
            .await?;
        o.envelope.name = name;
        set_folder(&mut o, parent);
        self.transact(steps, self.save_folder(o)).await
    }

    pub(super) async fn do_delete_folder(&self, id: ObjectId) -> Result<()> {
        let _w = self.lock.lock().await;
        let o = self.folder_obj(id).await?;
        if self.count_in(id).await? > 0 {
            return Err(Error::Conflict(
                Note::from(
                    "the folder is not empty (files in the trash count; purge them or move them out first)",
                )
                .about(Kind::Folder, id),
            ));
        }
        let parent = FolderRecord::of(&o)?.parent;
        let path = self.path_in(parent, &o.envelope.name).await?;
        let steps = self.named_steps(|| Disk::RemoveDir(path.clone()), true);
        let db = async {
            self.objects.delete(id, &self.actor).await?;
            Ok(())
        };
        self.transact(steps, db).await
    }

    /// The directory renames that go with moving or renaming a folder (one per disk that keeps real names).
    async fn move_steps(
        &self,
        from_parent: Option<fs_objects::ObjectId>,
        from_name: &str,
        to_parent: Option<fs_objects::ObjectId>,
        to_name: &str,
    ) -> Result<Vec<super::steps::Step>> {
        if self.backends.named().is_empty() {
            return Ok(Vec::new());
        }
        let from = self.path_in(from_parent, from_name).await?;
        let to = self.path_in(to_parent, to_name).await?;
        if from == to {
            return Ok(Vec::new());
        }
        let op = || Disk::MoveDir(from.clone(), to.clone());
        Ok(self.named_steps(op, false))
    }
}
