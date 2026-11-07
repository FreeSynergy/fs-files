//! `Folders` interface: folders, files, trash, rescan.
//!
//! Pattern: Facade — the generated trait has one method per call of the contract; each hands over to
//! the function of its concern (`folder_ops`, `file_ops`, `trash_ops`). It depends on the `Files`
//! trait only, never on `FileRepo` (ADR 0043).

use std::rc::Rc;
use std::sync::Arc;

use fs_files_core::Files;
use fs_interfaces::files_capnp::folders as f;

use crate::{file_ops, folder_ops, trash_ops};

pub(crate) struct FoldersImpl {
    pub(crate) files: Arc<dyn Files>,
}

type Done = Result<(), capnp::Error>;

impl f::Server for FoldersImpl {
    async fn list(self: Rc<Self>, p: f::ListParams, r: f::ListResults) -> Done {
        folder_ops::list(&self.files, p, r).await
    }
    async fn create_folder(
        self: Rc<Self>,
        p: f::CreateFolderParams,
        r: f::CreateFolderResults,
    ) -> Done {
        folder_ops::create(&self.files, p, r).await
    }
    async fn rename_folder(
        self: Rc<Self>,
        p: f::RenameFolderParams,
        r: f::RenameFolderResults,
    ) -> Done {
        folder_ops::rename(&self.files, p, r).await
    }
    async fn delete_folder(
        self: Rc<Self>,
        p: f::DeleteFolderParams,
        _: f::DeleteFolderResults,
    ) -> Done {
        folder_ops::delete(&self.files, p).await
    }
    async fn move_file(self: Rc<Self>, p: f::MoveFileParams, r: f::MoveFileResults) -> Done {
        file_ops::move_to(&self.files, p, r).await
    }
    async fn copy_file(self: Rc<Self>, p: f::CopyFileParams, r: f::CopyFileResults) -> Done {
        file_ops::copy(&self.files, p, r).await
    }
    async fn rename_file(self: Rc<Self>, p: f::RenameFileParams, r: f::RenameFileResults) -> Done {
        file_ops::rename(&self.files, p, r).await
    }
    async fn trash(self: Rc<Self>, p: f::TrashParams, _: f::TrashResults) -> Done {
        trash_ops::trash(&self.files, p).await
    }
    async fn restore(self: Rc<Self>, p: f::RestoreParams, r: f::RestoreResults) -> Done {
        trash_ops::restore(&self.files, p, r).await
    }
    async fn purge(self: Rc<Self>, p: f::PurgeParams, _: f::PurgeResults) -> Done {
        trash_ops::purge(&self.files, p).await
    }
    async fn get(self: Rc<Self>, p: f::GetParams, r: f::GetResults) -> Done {
        file_ops::get(&self.files, p, r).await
    }
    async fn classify(self: Rc<Self>, p: f::ClassifyParams, r: f::ClassifyResults) -> Done {
        file_ops::classify(&self.files, p, r).await
    }
    async fn move_folder(self: Rc<Self>, p: f::MoveFolderParams, r: f::MoveFolderResults) -> Done {
        folder_ops::move_to(&self.files, p, r).await
    }
    async fn restore_to(self: Rc<Self>, p: f::RestoreToParams, r: f::RestoreToResults) -> Done {
        trash_ops::restore_to(&self.files, p, r).await
    }
    async fn empty_trash(self: Rc<Self>, _: f::EmptyTrashParams, r: f::EmptyTrashResults) -> Done {
        trash_ops::empty(&self.files, r).await
    }
    async fn get_folder(self: Rc<Self>, p: f::GetFolderParams, r: f::GetFolderResults) -> Done {
        folder_ops::get(&self.files, p, r).await
    }
    async fn rescan(self: Rc<Self>, p: f::RescanParams, r: f::RescanResults) -> Done {
        folder_ops::rescan(&self.files, p, r).await
    }
}
