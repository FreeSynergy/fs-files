//! The trait implementations of [`FileRepo`] and its capability report.
//!
//! Pattern: Facade — `Files`, `Transfer` and `Backends` forward to the one-concept files next to
//! this one. `capabilities` says plainly what is not built (ADR 0045).

use std::sync::Arc;

use async_trait::async_trait;
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

use super::fileops::Target;
use super::handle::FileRepo;
use crate::backends::Backends;
use crate::error::Result;
use crate::files::Files;
use crate::model::{
    BackendInfo, Cursor, File, FileFilter, Folder, OnConflict, Page, RescanReport, Sort,
    TrashReport,
};
use crate::transfer::{Blob, Transfer, Upload};

const SUPPORTED: [&str; 24] = [
    "file.resolve",
    "folder.list",
    "folder.create",
    "folder.rename",
    "folder.move",
    "folder.delete",
    "folder.rescan",
    "file.conflictRule",
    "file.sort",
    "file.restoreTo",
    "file.emptyTrash",
    "file.move",
    "file.copy",
    "file.rename",
    "file.delete",
    "file.restore",
    "file.purge",
    "file.classify",
    "file.search",
    "file.upload",
    "file.download",
    "backend.list",
    "backend.add",
    "backend.remove",
];

const NOT_BUILT: [(&str, &str); 6] = [
    (
        "share.create",
        "one specification for all programs, not written yet",
    ),
    (
        "share.revoke",
        "one specification for all programs, not written yet",
    ),
    ("file.subscribe", "not built yet; it is possible"),
    ("file.versions", "not built yet"),
    ("file.encrypt", "not built yet"),
    ("sync.configure", "not built yet"),
];

#[async_trait]
impl Files for FileRepo {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("fs-files-core");
        for id in SUPPORTED {
            c.supported(id);
        }
        for (id, why) in NOT_BUILT {
            c.unsupported(id, why);
        }
        c.unsupported("file.preview", "not built yet");
        c
    }
    async fn get(&self, id: ObjectId) -> Result<Option<File>> {
        match self.file_obj(id).await {
            Ok(o) => Ok(Some(File::of(&o)?)),
            Err(crate::Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn get_folder(&self, id: ObjectId) -> Result<Option<Folder>> {
        match self.folder_obj(id).await {
            Ok(o) => Ok(Some(Folder::of(&o)?)),
            Err(crate::Error::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
    async fn list(&self, folder: Option<ObjectId>, cursor: Option<Cursor>) -> Result<Page> {
        self.do_list(folder, cursor, Sort::default()).await
    }
    async fn list_sorted(
        &self,
        folder: Option<ObjectId>,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        self.do_list(folder, cursor, sort).await
    }
    async fn create_folder(&self, parent: Option<ObjectId>, name: &str) -> Result<Folder> {
        self.do_create_folder(parent, name).await
    }
    async fn rename_folder(&self, id: ObjectId, name: &str) -> Result<Folder> {
        self.do_rename_folder(id, name, OnConflict::Refuse).await
    }
    async fn rename_folder_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<Folder> {
        self.do_rename_folder(id, name, on).await
    }
    async fn move_folder(&self, id: ObjectId, parent: Option<ObjectId>) -> Result<Folder> {
        self.do_move_folder(id, parent, OnConflict::Refuse).await
    }
    async fn move_folder_as(
        &self,
        id: ObjectId,
        parent: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<Folder> {
        self.do_move_folder(id, parent, on).await
    }
    async fn delete_folder(&self, id: ObjectId) -> Result<()> {
        self.do_delete_folder(id).await
    }
    async fn move_file(&self, id: ObjectId, folder: Option<ObjectId>) -> Result<File> {
        self.do_move_file(id, folder, OnConflict::Refuse).await
    }
    async fn move_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.do_move_file(id, folder, on).await
    }
    async fn copy_file(&self, id: ObjectId, folder: Option<ObjectId>) -> Result<File> {
        self.do_copy_file(id, folder, OnConflict::KeepBoth).await
    }
    async fn copy_file_as(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.do_copy_file(id, folder, on).await
    }
    async fn rename_file(&self, id: ObjectId, name: &str) -> Result<File> {
        self.do_rename_file(id, name, OnConflict::Refuse).await
    }
    async fn rename_file_as(&self, id: ObjectId, name: &str, on: OnConflict) -> Result<File> {
        self.do_rename_file(id, name, on).await
    }
    async fn trash(&self, id: ObjectId) -> Result<()> {
        self.do_trash(id).await
    }
    async fn restore(&self, id: ObjectId) -> Result<File> {
        self.do_restore(id, Target::Original, OnConflict::Refuse)
            .await
    }
    async fn restore_as(&self, id: ObjectId, on: OnConflict) -> Result<File> {
        self.do_restore(id, Target::Original, on).await
    }
    async fn restore_to(
        &self,
        id: ObjectId,
        folder: Option<ObjectId>,
        on: OnConflict,
    ) -> Result<File> {
        self.do_restore(id, Target::Folder(folder), on).await
    }
    async fn empty_trash(&self) -> Result<TrashReport> {
        self.do_empty_trash().await
    }
    async fn purge(&self, id: ObjectId) -> Result<()> {
        self.do_purge(id).await
    }
    async fn classify(&self, id: ObjectId, categories: Vec<String>) -> Result<File> {
        self.do_classify(id, categories).await
    }
    async fn find(&self, filter: &FileFilter, cursor: Option<Cursor>) -> Result<Page> {
        self.do_find(filter, cursor, Sort::default()).await
    }
    async fn find_sorted(
        &self,
        filter: &FileFilter,
        cursor: Option<Cursor>,
        sort: Sort,
    ) -> Result<Page> {
        self.do_find(filter, cursor, sort).await
    }
    async fn rescan(&self, folder: Option<ObjectId>) -> Result<RescanReport> {
        self.do_rescan(folder).await
    }
}

#[async_trait]
impl Transfer for FileRepo {
    async fn open(&self, id: ObjectId) -> Result<Arc<dyn Blob>> {
        self.do_open(id).await
    }
    async fn start_upload(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
    ) -> Result<(Arc<dyn Upload>, String)> {
        self.do_start_upload(folder, name, media_type, OnConflict::Refuse)
            .await
    }
    async fn start_upload_as(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        media_type: &str,
        on: OnConflict,
    ) -> Result<(Arc<dyn Upload>, String)> {
        self.do_start_upload(folder, name, media_type, on).await
    }
    async fn resume_upload(&self, id: &str) -> Result<Arc<dyn Upload>> {
        self.do_resume_upload(id).await
    }
}

#[async_trait]
impl Backends for FileRepo {
    async fn list_backends(&self) -> Result<Vec<BackendInfo>> {
        Ok(self.backends.list())
    }
    async fn add_backend(&self, config: &str) -> Result<BackendInfo> {
        self.do_add_backend(config).await
    }
    async fn remove_backend(&self, id: &str) -> Result<()> {
        self.do_remove_backend(id).await
    }
}

impl FileRepo {
    /// Id of the backend that holds the bytes of a file (for operators and tests; callers of the contract never need it).
    pub async fn backend_of(&self, id: ObjectId) -> Result<String> {
        Ok(crate::model::FileRecord::of(&self.file_obj(id).await?)?.backend)
    }
}
