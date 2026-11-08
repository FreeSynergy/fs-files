//! The `FileSource` impl of [`FilesRemote`]: delegates to the call modules, answers `Unsupported` for the rest.
//!
//! Pattern: Adapter — implements the workbench's trait over the remote; no logic of its own.

use fs_components_workbenches::file_manager::{
    EmptyReport, EntryPage, EntryRef, FileCapabilities, FileEntry, FileError, FileSource,
    FolderInfo, ListRequest, NameRule, PageMark, PendingUpload, PreviewBytes, RescanSummary,
    TrashCount,
};

use crate::remote::FilesRemote;

impl FileSource for FilesRemote {
    fn capabilities(&self) -> FileCapabilities {
        let mut can = FileCapabilities::all();
        // What this window does not do yet is not offered.
        can.upload = false;
        can.download = false;
        can.type_descending = false;
        can.dates = false;
        can
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

    fn preview(&self, _key: &str, _limit: usize) -> Result<PreviewBytes, FileError> {
        Err(FileError::unsupported("this window shows no previews yet"))
    }

    fn upload(
        &self,
        _path: &str,
        _folder: Option<&str>,
        _rule: NameRule,
    ) -> Result<String, FileError> {
        Err(FileError::unsupported("this window takes no uploads yet"))
    }

    fn pending_uploads(&self) -> Result<Vec<PendingUpload>, FileError> {
        Ok(Vec::new())
    }

    fn resume_upload(&self, _id: &str, _path: &str) -> Result<String, FileError> {
        Err(FileError::unsupported("this window takes no uploads yet"))
    }

    fn drop_upload(&self, _id: &str) -> Result<(), FileError> {
        Err(FileError::unsupported("this window takes no uploads yet"))
    }

    fn download(&self, _key: &str, _path: &str, _release: bool) -> Result<String, FileError> {
        Err(FileError::unsupported(
            "this window writes no downloads yet",
        ))
    }
}
