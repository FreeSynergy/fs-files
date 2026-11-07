//! The adapter as a `FileSource`.
//!
//! Pattern: Adapter — one line per method; the work is in the files beside this one. What the workbench
//! offers follows from what the core reports (ADR 0045), by the ids of the contract.

use fs_components_workbenches::file_manager::{
    EmptyReport, EntryPage, EntryRef, FileCapabilities, FileEntry, FileError, FileSource,
    FolderInfo, ListRequest, NameRule, PageMark, PendingUpload, PreviewBytes, RescanSummary,
    Transferred, TrashCount,
};

use super::adapter::{FileAdapter, id_of};

impl FileSource for FileAdapter {
    fn capabilities(&self) -> FileCapabilities {
        let report = self.files.capabilities();
        let has = |id: &str| report.is_supported(id);
        let mut caps = FileCapabilities::all();
        caps.change =
            has("folder.create") && has("file.rename") && has("file.move") && has("file.copy");
        caps.keep_both = has("file.conflictRule");
        caps.trash = has("file.delete")
            && has("file.restore")
            && has("file.purge")
            && has("file.emptyTrash");
        caps.restore_to = has("file.restoreTo");
        caps.upload = has("file.upload") && has("file.pendingUploads");
        caps.download = has("file.download");
        caps.rescan = has("folder.rescan");
        caps.sort = has("file.sort");
        // The contract: by type is ascending only. A file carries no date yet (`DESIGN.md`).
        caps.type_descending = false;
        caps.search = has("file.search");
        caps.dates = false;
        caps
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
        self.do_move(entry, folder, rule)
    }

    fn copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        self.do_copy(key, folder, rule)
    }

    fn trash(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.wait(self.files.trash(id))
    }

    fn delete_folder(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.wait(self.files.delete_folder(id))
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
        let id = id_of(key)?;
        self.wait(self.files.purge(id))
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
    ) -> Result<Transferred, FileError> {
        self.do_upload(path, folder, rule)
    }

    fn pending_uploads(&self) -> Result<Vec<PendingUpload>, FileError> {
        self.do_pending()
    }

    fn resume_upload(&self, id: &str, path: &str) -> Result<Transferred, FileError> {
        self.do_resume(id, path)
    }

    fn drop_upload(&self, id: &str) -> Result<(), FileError> {
        self.do_drop(id)
    }

    fn download(
        &self,
        key: &str,
        path: &str,
        release_marked: bool,
    ) -> Result<Transferred, FileError> {
        self.do_download(key, path, release_marked)
    }
}
