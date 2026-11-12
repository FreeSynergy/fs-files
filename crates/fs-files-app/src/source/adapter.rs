//! The files server as a `FileSource` of the file workbench (ADR 0066: the program only wires).
//!
//! Pattern: Adapter — `files.capnp` behind the `FileSource` trait of `fs-components-workbenches`; the keys the
//! workbench holds are the bare ids of the records. Every call is a job of the worker of [`FilesRemote`] with a
//! limit, so a server that does not answer ends in a visible error, never in a window that waits for ever. The trait
//! is synchronous: the workbench (fs-components v0.0.12) calls the long ones (rescan, upload, download) from a worker
//! and the short ones on the interface thread, each with its limit.

use std::sync::Arc;

use fs_admin_frontend::BackendError;
use fs_components_workbenches::file_manager::{
    EmptyReport, EntryPage, EntryRef, FileCapabilities, FileEntry, FileError, FileSource,
    FolderInfo, ListRequest, NameRule, PageMark, PendingUpload, PreviewBytes, RescanSummary,
    TrashCount,
};

use crate::remote::{Clients, FilesRemote};

use super::convert::error_of;

/// The files server as the source of the workbench.
pub struct RemoteFileSource(pub Arc<FilesRemote>);

impl RemoteFileSource {
    pub(super) fn within<T, F, Fut>(&self, command: &str, make: F) -> Result<T, FileError>
    where
        T: Send + 'static,
        F: FnOnce(std::rc::Rc<Clients>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, BackendError>> + 'static,
    {
        self.0
            .run_within(self.0.limit_of(command), make)
            .map_err(error_of)
    }

    pub(super) fn run<T, F, Fut>(&self, make: F) -> Result<T, FileError>
    where
        T: Send + 'static,
        F: FnOnce(std::rc::Rc<Clients>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, BackendError>> + 'static,
    {
        self.0.run(make).map_err(error_of)
    }
}

impl FileSource for RemoteFileSource {
    fn capabilities(&self) -> FileCapabilities {
        let mut all = FileCapabilities::all();
        all.type_descending = false;
        all
    }

    fn list(
        &self,
        request: &ListRequest,
        after: Option<&PageMark>,
    ) -> Result<EntryPage, FileError> {
        self.do_list(request, after)
    }

    fn entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
        self.do_entry(reference)
    }

    fn folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
        self.do_folder(key)
    }

    fn subfolders(
        &self,
        parent: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<FolderInfo>, bool), FileError> {
        self.do_subfolders(parent, limit)
    }

    fn preview(&self, key: &str, limit: usize) -> Result<PreviewBytes, FileError> {
        self.do_preview(key, limit)
    }

    fn create_folder(&self, parent: Option<&str>, name: &str) -> Result<FolderInfo, FileError> {
        self.do_create_folder(parent, name)
    }

    fn rename(&self, entry: &EntryRef, name: &str, rule: NameRule) -> Result<String, FileError> {
        self.do_rename(entry, name, rule)
    }

    fn move_to(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        self.do_move_to(entry, folder, rule)
    }

    fn copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        self.do_copy_to(key, folder, rule)
    }

    fn trash(&self, key: &str) -> Result<(), FileError> {
        self.do_trash(key)
    }

    fn delete_folder(&self, key: &str) -> Result<(), FileError> {
        self.do_delete_folder(key)
    }

    fn restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        self.do_restore(key, rule)
    }

    fn restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        self.do_restore_to(key, folder, rule)
    }

    fn purge(&self, key: &str) -> Result<(), FileError> {
        self.do_purge(key)
    }

    fn trash_count(&self) -> Result<TrashCount, FileError> {
        self.do_trash_count()
    }

    fn empty_trash(&self) -> Result<EmptyReport, FileError> {
        self.do_empty_trash()
    }

    fn rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        self.do_rescan(folder)
    }

    fn upload(
        &self,
        path: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        self.do_upload(path, folder, rule)
    }

    fn pending_uploads(&self) -> Result<Vec<PendingUpload>, FileError> {
        self.do_pending_uploads()
    }

    fn resume_upload(&self, _id: &str, _path: &str) -> Result<String, FileError> {
        Err(FileError::unsupported(
            "going on with an unfinished upload is not possible yet",
        ))
    }

    fn drop_upload(&self, id: &str) -> Result<(), FileError> {
        self.do_drop_upload(id)
    }

    fn download(&self, key: &str, path: &str, _: bool) -> Result<String, FileError> {
        self.do_download(key, path)
    }
}
