//! `Files` over the socket: the methods the app calls.
//!
//! Pattern: Adapter — each method is one call of `Folders`, `Search` or the root. `classify` and
//! `sharing` are not overridden, so they answer the `Unsupported` of the trait's defaults (ADR 0045);
//! `capabilities()` says the same (`peer.rs`), and `DESIGN.md` lists them. The plain methods (`move_file`
//! and the others without a rule) are the same calls with the refuse rule, so that what
//! `capabilities()` reports as possible is possible.

use async_trait::async_trait;
use fs_files_core::{
    Cursor, File, FileFilter, Files, Folder, OnConflict, Page, RescanReport, Result, Sort,
    TrashReport,
};
use fs_interfaces::Capabilities;
use fs_objects::ObjectId;

use crate::RpcFiles;
use crate::file_calls::{self as file, Change};
use crate::folder_calls as folders;

#[async_trait]
impl Files for RpcFiles {
    fn capabilities(&self) -> Capabilities {
        self.report()
    }

    async fn get(&self, id: ObjectId) -> Result<Option<File>> {
        self.call(move |r| async move { folders::get(&r.folders, id).await })
            .await
    }

    async fn get_folder(&self, id: ObjectId) -> Result<Option<Folder>> {
        self.call(move |r| async move { folders::get_folder(&r.folders, id).await })
            .await
    }

    async fn list(&self, folder: Option<ObjectId>, cursor: Option<Cursor>) -> Result<Page> {
        self.list_sorted(folder, cursor, Sort::default()).await
    }

    async fn list_sorted(
        &self,
        folder: Option<ObjectId>,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        self.call(move |r| async move { folders::list(&r.folders, folder, cursor, sort).await })
            .await
    }

    async fn create_folder(&self, parent: Option<ObjectId>, name: &str) -> Result<Folder> {
        let name = name.to_string();
        self.call(move |r| async move { folders::create_folder(&r.folders, parent, name).await })
            .await
    }

    async fn rename_folder(&self, id: ObjectId, name: &str) -> Result<Folder> {
        self.rename_folder_as(id, name, OnConflict::Refuse).await
    }

    async fn move_folder(&self, id: ObjectId, parent: Option<ObjectId>) -> Result<Folder> {
        self.move_folder_as(id, parent, OnConflict::Refuse).await
    }

    async fn rename_folder_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<Folder> {
        let name = name.to_string();
        self.call(move |r| async move { folders::rename_folder(&r.folders, id, name, on).await })
            .await
    }

    async fn move_folder_as(
        &self,
        id: ObjectId,
        parent: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<Folder> {
        self.call(move |r| async move { folders::move_folder(&r.folders, id, parent, on).await })
            .await
    }

    async fn delete_folder(&self, id: ObjectId) -> Result<()> {
        self.call(move |r| async move { folders::delete_folder(&r.folders, id).await })
            .await
    }

    async fn move_file(&self, id: ObjectId, folder: Option<ObjectId>) -> Result<File> {
        self.move_file_as(id, folder, OnConflict::Refuse).await
    }

    async fn copy_file(&self, id: ObjectId, folder: Option<ObjectId>) -> Result<File> {
        self.copy_file_as(id, folder, OnConflict::KeepBoth).await
    }

    async fn rename_file(&self, id: ObjectId, name: &str) -> Result<File> {
        self.rename_file_as(id, name, OnConflict::Refuse).await
    }

    async fn move_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.change(id, Change::Move(folder), on).await
    }

    async fn copy_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.change(id, Change::Copy(folder), on).await
    }

    async fn rename_file_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<File> {
        self.change(id, Change::Rename(name.to_string()), on).await
    }

    async fn trash(&self, id: ObjectId) -> Result<()> {
        self.call(move |r| async move { file::trash(&r.folders, id).await })
            .await
    }

    async fn restore(&self, id: ObjectId) -> Result<File> {
        self.restore_as(id, OnConflict::Refuse).await
    }

    async fn restore_as(&self, id: ObjectId, on: OnConflict) -> Result<File> {
        self.change(id, Change::Restore, on).await
    }

    async fn restore_to(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.change(id, Change::RestoreTo(folder), on).await
    }

    async fn empty_trash(&self) -> Result<TrashReport> {
        self.call(move |r| async move { folders::empty_trash(&r.folders).await })
            .await
    }

    async fn purge(&self, id: ObjectId) -> Result<()> {
        self.call(move |r| async move { file::purge(&r.folders, id).await })
            .await
    }

    async fn find(&self, filter: &FileFilter, cursor: Option<Cursor>) -> Result<Page> {
        self.find_sorted(filter, cursor, Sort::default()).await
    }

    async fn find_sorted(
        &self,
        filter: &FileFilter,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        let filter = filter.clone();
        self.call(move |r| async move { file::find(&r.search, filter, cursor, sort).await })
            .await
    }

    async fn rescan(&self, folder: Option<ObjectId>) -> Result<RescanReport> {
        self.call(move |r| async move { folders::rescan(&r.folders, folder).await })
            .await
    }
}

impl RpcFiles {
    async fn change(&self, id: ObjectId, change: Change, on: OnConflict) -> Result<File> {
        self.call(move |r| async move { file::change(&r.folders, id, change, on).await })
            .await
    }
}
