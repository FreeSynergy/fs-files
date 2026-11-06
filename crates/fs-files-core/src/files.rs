//! The [`Files`] contract: folders, files, trash, classification, search.
//!
//! Pattern: Repository (interface) + Null Object — callers depend on this trait, never on the
//! store-backed `FileRepo`. Like a parent class (ADR 0045): every provider has every method; one
//! it does not override answers an explicit `Unsupported`. The Rust form of `Files`, `Folders` and
//! `Search` in `files.capnp`. Bytes are in [`Transfer`](crate::Transfer), backends in [`Backends`](crate::Backends).

use async_trait::async_trait;
use fs_interfaces::{Capabilities, Unsupported};
use fs_store::ObjectId;

use crate::error::Result;
use crate::model::{Cursor, File, FileFilter, Folder, OnConflict, Page, RescanReport};

const NO_RULE: &str = "this provider only knows the refuse rule";

fn no<T>(capability: &str, why: &str) -> Result<T> {
    Err(Unsupported::new(capability, why).into())
}

/// Files and folders.
///
/// Interface status: draft (ADR 0045).
#[async_trait]
pub trait Files: Send + Sync {
    /// What this provider can do, per capability id (also for [`Transfer`](crate::Transfer) and [`Backends`](crate::Backends)).
    fn capabilities(&self) -> Capabilities;
    /// `file.resolve`: a file by id, trashed or not.
    async fn get(&self, id: ObjectId) -> Result<Option<File>>;
    /// A folder by id.
    async fn get_folder(&self, id: ObjectId) -> Result<Option<Folder>>;
    /// `folder.list`: the live entries of a folder (`None` = top level) in name order.
    async fn list(&self, folder: Option<ObjectId>, cursor: Option<Cursor>) -> Result<Page>;

    /// `folder.create`: a folder inside `parent` (`None` = top level).
    async fn create_folder(&self, _parent: Option<ObjectId>, _name: &str) -> Result<Folder> {
        no("folder.create", "this provider cannot create folders")
    }
    /// `folder.rename`.
    async fn rename_folder(&self, _id: ObjectId, _name: &str) -> Result<Folder> {
        no("folder.rename", "this provider cannot rename folders")
    }
    /// `folder.move`: move a folder; into itself or one of its descendants is refused. Not in the contract yet.
    async fn move_folder(&self, _id: ObjectId, _parent: Option<ObjectId>) -> Result<Folder> {
        no("folder.move", "this provider cannot move folders")
    }
    /// `folder.rename` with a rule for a taken name (`onConflict` in the contract). The default refuses.
    async fn rename_folder_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<Folder> {
        match on {
            OnConflict::Refuse => self.rename_folder(id, name).await,
            OnConflict::KeepBoth => no("file.conflictRule", NO_RULE),
        }
    }
    /// `folder.move` with a rule for a taken name. The default refuses.
    async fn move_folder_as(
        &self,
        id: ObjectId,
        parent: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<Folder> {
        match on {
            OnConflict::Refuse => self.move_folder(id, parent).await,
            OnConflict::KeepBoth => no("file.conflictRule", NO_RULE),
        }
    }
    /// `folder.delete`: only an empty folder (nothing live, nothing in the trash).
    async fn delete_folder(&self, _id: ObjectId) -> Result<()> {
        no("folder.delete", "this provider cannot delete folders")
    }
    /// `file.move`.
    async fn move_file(&self, _id: ObjectId, _folder: Option<ObjectId>) -> Result<File> {
        no("file.move", "this provider cannot move files")
    }
    /// `file.copy`: a new file with copied bytes; a taken name becomes `name (copy)`.
    async fn copy_file(&self, _id: ObjectId, _folder: Option<ObjectId>) -> Result<File> {
        no("file.copy", "this provider cannot copy files")
    }
    /// `file.rename`.
    async fn rename_file(&self, _id: ObjectId, _name: &str) -> Result<File> {
        no("file.rename", "this provider cannot rename files")
    }
    /// `file.move` with a rule for a taken name. The default refuses.
    async fn move_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        match on {
            OnConflict::Refuse => self.move_file(id, folder).await,
            OnConflict::KeepBoth => no("file.conflictRule", NO_RULE),
        }
    }
    /// `file.copy` with a rule for a taken name. Unlike `copy_file`, the default is to refuse: a
    /// frontend that duplicates a file sends `KeepBoth`.
    async fn copy_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        match on {
            OnConflict::KeepBoth => self.copy_file(id, folder).await,
            OnConflict::Refuse => no("file.conflictRule", NO_RULE),
        }
    }
    /// `file.rename` with a rule for a taken name. The default refuses.
    async fn rename_file_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<File> {
        match on {
            OnConflict::Refuse => self.rename_file(id, name).await,
            OnConflict::KeepBoth => no("file.conflictRule", NO_RULE),
        }
    }
    /// `file.delete`: put a file in the trash; it is hidden, nothing is deleted.
    async fn trash(&self, _id: ObjectId) -> Result<()> {
        no("file.delete", "this provider cannot trash files")
    }
    /// `file.restore`: bring a file back from the trash.
    async fn restore(&self, _id: ObjectId) -> Result<File> {
        no("file.restore", "this provider cannot restore files")
    }
    /// `file.purge`: delete bytes and metadata of a trashed file for good.
    async fn purge(&self, _id: ObjectId) -> Result<()> {
        no("file.purge", "this provider cannot purge files")
    }
    /// `file.classify`: set the sensitive categories (ADR 0029); an empty list clears them.
    async fn classify(&self, _id: ObjectId, _categories: Vec<String>) -> Result<File> {
        no("file.classify", "this provider cannot classify files")
    }
    /// `file.search`: files and folders that match all members of the filter.
    async fn find(&self, _filter: &FileFilter, _cursor: Option<Cursor>) -> Result<Page> {
        no("file.search", "this provider cannot search")
    }
    /// `folder.rescan`: look at the disk below `folder` (`None` = the top) and bring the records in line with
    /// it: take over new files and folders, update size and checksum of changed files, and **report**
    /// the vanished ones without deleting their records. Needs a backend that keeps real names.
    async fn rescan(&self, _folder: Option<ObjectId>) -> Result<RescanReport> {
        no(
            "folder.rescan",
            "this provider has no folder that people can change from outside",
        )
    }
    /// `share.create` / `share.revoke`: placeholder, the contract has no methods yet.
    async fn sharing(&self) -> Result<()> {
        no(
            "share.create",
            "one specification for all programs, not written yet",
        )
    }
    /// `file.subscribe`: live changes for other programs. The contract takes a listener; this
    /// placeholder has none because nothing can be built on it yet.
    async fn subscribe(&self) -> Result<()> {
        no("file.subscribe", "not built yet; it is possible")
    }
}
