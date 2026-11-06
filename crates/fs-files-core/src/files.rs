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
use crate::model::{Cursor, File, FileFilter, Folder, Page};

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
