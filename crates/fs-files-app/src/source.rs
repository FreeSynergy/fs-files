//! `FileSource` for [`FilesRemote`]: the file manager workbench over `files.capnp`.
//!
//! Pattern: Adapter — each method is a few contract calls on the worker thread; the server's problems
//! come back as the workbench's [`FileProblem`] kinds with the server's own words. What this window
//! does not do yet answers `Unsupported` and is not offered (capabilities): previews, upload, download.
//! A sensitive marking of a file (ADR 0029) is not read yet, so entries show none.

use std::sync::Arc;

use fs_components_workbenches::file_manager::{
    EmptyReport, EntryKind, EntryPage, EntryRef, FileCapabilities, FileEntry, FileError,
    FileSource, FolderInfo, ListRequest, NameRule, PageMark, PendingUpload, PreviewBytes,
    RescanSummary, Scope, SortKey, TrashCount,
};
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::{OnConflict, SortBy, entry, file, folder, folders, search};
use fs_interfaces::rpc::{read_envelope, read_uuid, write_uuid};
use uuid::Uuid;

use crate::remote::{FilesRemote, problem};

fn id(key: &str) -> Result<Uuid, FileError> {
    key.parse().map_err(|_| {
        FileError::new(
            fs_components_workbenches::file_manager::FileProblem::NotFound,
            format!("\"{key}\" names nothing."),
        )
    })
}

fn optional(key: Option<&str>) -> Result<Option<Uuid>, FileError> {
    key.map(id).transpose()
}

fn on_conflict(rule: NameRule) -> OnConflict {
    match rule {
        NameRule::KeepBoth => OnConflict::KeepBoth,
        _ => OnConflict::Refuse,
    }
}

fn sort_by(key: SortKey) -> SortBy {
    match key {
        SortKey::Modified => SortBy::Modified,
        SortKey::Size => SortBy::Size,
        SortKey::Type => SortBy::MediaType,
        _ => SortBy::Name,
    }
}

fn file_of(f: file::Reader<'_>) -> capnp::Result<FileEntry> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut e = FileEntry::file(envelope.id.to_string(), envelope.name, f.get_size());
    e.media_type = f.get_media_type()?.to_string()?;
    e.modified = Some(envelope.updated.to_rfc3339());
    e.folder = read_uuid(f.get_folder()?).map(|u| u.to_string());
    e.trashed = f.get_trashed();
    Ok(e)
}

fn folder_of(f: folder::Reader<'_>) -> capnp::Result<FileEntry> {
    let envelope = read_envelope(f.get_envelope()?)?;
    let mut e = FileEntry::folder(envelope.id.to_string(), envelope.name);
    e.modified = Some(envelope.updated.to_rfc3339());
    e.folder = read_uuid(f.get_parent()?).map(|u| u.to_string());
    Ok(e)
}

fn entry_of(e: entry::Reader<'_>) -> capnp::Result<FileEntry> {
    match e.which()? {
        entry::Which::File(f) => file_of(f?),
        entry::Which::Folder(f) => folder_of(f?),
    }
}

/// The cursor a page mark stands for; the first page has none.
fn cursor_of(after: Option<&PageMark>) -> Vec<u8> {
    after
        .and_then(|m| m.0.downcast_ref::<Vec<u8>>())
        .cloned()
        .unwrap_or_default()
}

fn page_of(
    items: capnp::struct_list::Reader<'_, entry::Owned>,
    next: &[u8],
) -> capnp::Result<EntryPage> {
    let mut entries = Vec::new();
    for item in items {
        entries.push(entry_of(item)?);
    }
    let mark = (!next.is_empty()).then(|| PageMark(Arc::new(next.to_vec())));
    Ok(EntryPage::new(entries, mark))
}

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

    #[allow(clippy::single_match_else)] // two ways to the same page: one folder, or a search
    fn list(
        &self,
        request: &ListRequest,
        after: Option<&PageMark>,
    ) -> Result<EntryPage, FileError> {
        let cursor = cursor_of(after);
        let sort = request.sort;
        let plain = request.term.trim().is_empty() && !request.filter.is_active();
        match (&request.scope, plain) {
            (Scope::Folder(folder), true) => {
                let folder = optional(folder.as_deref())?;
                self.run(move |c| async move {
                    let mut call = c.folders.list_request();
                    write_uuid(call.get().init_folder(), folder);
                    call.get().init_cursor().set_opaque(&cursor);
                    {
                        let mut s = call.get().init_sort();
                        s.set_by(sort_by(sort.key));
                        s.set_descending(sort.descending);
                    }
                    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                    let page = answer
                        .get()
                        .and_then(folders::list_results::Reader::get_page)
                        .map_err(|e| problem(&e))?;
                    let next = page
                        .get_next()
                        .and_then(common_capnp::cursor::Reader::get_opaque)
                        .map_err(|e| problem(&e))?;
                    page_of(page.get_items().map_err(|e| problem(&e))?, next)
                        .map_err(|e| problem(&e))
                })
            }
            _ => {
                let (term, scope, filter) = (
                    request.term.trim().to_string(),
                    request.scope.clone(),
                    request.filter.clone(),
                );
                self.run(move |c| async move {
                    let mut call = c.search.find_request();
                    {
                        let mut f = call.get().init_filter();
                        f.set_name(term.as_str());
                        match &scope {
                            Scope::Trash => f.set_trashed(true),
                            Scope::Folder(Some(folder)) => {
                                write_uuid(f.reborrow().init_folder(), folder.parse().ok());
                            }
                            _ => {}
                        }
                        if let Some(min) = filter.min_size {
                            f.set_min_size(min);
                        }
                        if let Some(max) = filter.max_size {
                            f.set_max_size(i64::try_from(max).unwrap_or(i64::MAX));
                        }
                    }
                    call.get().init_cursor().set_opaque(&cursor);
                    {
                        let mut s = call.get().init_sort();
                        s.set_by(sort_by(sort.key));
                        s.set_descending(sort.descending);
                    }
                    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                    let page = answer
                        .get()
                        .and_then(search::find_results::Reader::get_page)
                        .map_err(|e| problem(&e))?;
                    let next = page
                        .get_next()
                        .and_then(common_capnp::cursor::Reader::get_opaque)
                        .map_err(|e| problem(&e))?;
                    page_of(page.get_items().map_err(|e| problem(&e))?, next)
                        .map_err(|e| problem(&e))
                })
            }
        }
    }

    fn entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
        let (key, kind) = (id(&reference.key)?, reference.kind);
        self.run(move |c| async move {
            if kind == EntryKind::Folder {
                let mut call = c.folders.get_folder_request();
                write_uuid(call.get().init_id(), Some(key));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let r = answer.get().map_err(|e| problem(&e))?;
                if !r.get_found() {
                    return Ok(None);
                }
                return r
                    .get_folder()
                    .and_then(folder_of)
                    .map(Some)
                    .map_err(|e| problem(&e));
            }
            let mut call = c.folders.get_request();
            write_uuid(call.get().init_id(), Some(key));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let r = answer.get().map_err(|e| problem(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            r.get_file()
                .and_then(file_of)
                .map(Some)
                .map_err(|e| problem(&e))
        })
    }

    fn folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.get_folder_request();
            write_uuid(call.get().init_id(), Some(key));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let r = answer.get().map_err(|e| problem(&e))?;
            if !r.get_found() {
                return Ok(None);
            }
            let e = r
                .get_folder()
                .and_then(folder_of)
                .map_err(|e| problem(&e))?;
            Ok(Some(FolderInfo::new(e.key, e.name, e.folder)))
        })
    }

    fn subfolders(
        &self,
        parent: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<FolderInfo>, bool), FileError> {
        let parent = optional(parent)?;
        self.run(move |c| async move {
            let (mut found, mut cursor) = (Vec::new(), Vec::<u8>::new());
            loop {
                let mut call = c.folders.list_request();
                write_uuid(call.get().init_folder(), parent);
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(folders::list_results::Reader::get_page)
                    .map_err(|e| problem(&e))?;
                for item in page.get_items().map_err(|e| problem(&e))? {
                    let e = entry_of(item).map_err(|e| problem(&e))?;
                    if e.kind == EntryKind::Folder {
                        if found.len() == limit {
                            return Ok((found, true));
                        }
                        found.push(FolderInfo::new(e.key, e.name, e.folder));
                    }
                }
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    return Ok((found, false));
                }
                cursor = next.to_vec();
            }
        })
    }

    fn preview(&self, _key: &str, _limit: usize) -> Result<PreviewBytes, FileError> {
        Err(FileError::unsupported("this window shows no previews yet"))
    }

    fn create_folder(&self, parent: Option<&str>, name: &str) -> Result<FolderInfo, FileError> {
        let (parent, name) = (optional(parent)?, name.to_string());
        self.run(move |c| async move {
            let mut call = c.folders.create_folder_request();
            write_uuid(call.get().init_parent(), parent);
            call.get().set_name(name.as_str());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let e = answer
                .get()
                .and_then(folders::create_folder_results::Reader::get_folder)
                .and_then(folder_of)
                .map_err(|e| problem(&e))?;
            Ok(FolderInfo::new(e.key, e.name, e.folder))
        })
    }

    fn rename(&self, entry: &EntryRef, name: &str, rule: NameRule) -> Result<String, FileError> {
        let (key, kind, name) = (id(&entry.key)?, entry.kind, name.to_string());
        self.run(move |c| async move {
            if kind == EntryKind::Folder {
                let mut call = c.folders.rename_folder_request();
                write_uuid(call.get().init_id(), Some(key));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(on_conflict(rule));
                call.send().promise.await.map_err(|e| problem(&e))?;
            } else {
                let mut call = c.folders.rename_file_request();
                write_uuid(call.get().init_id(), Some(key));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(on_conflict(rule));
                call.send().promise.await.map_err(|e| problem(&e))?;
            }
            Ok(key.to_string())
        })
    }

    fn move_to(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, kind, to) = (id(&entry.key)?, entry.kind, optional(folder)?);
        self.run(move |c| async move {
            if kind == EntryKind::Folder {
                let mut call = c.folders.move_folder_request();
                write_uuid(call.get().init_id(), Some(key));
                write_uuid(call.get().init_parent(), to);
                call.get().set_on_conflict(on_conflict(rule));
                call.send().promise.await.map_err(|e| problem(&e))?;
            } else {
                let mut call = c.folders.move_file_request();
                write_uuid(call.get().init_id(), Some(key));
                write_uuid(call.get().init_folder(), to);
                call.get().set_on_conflict(on_conflict(rule));
                call.send().promise.await.map_err(|e| problem(&e))?;
            }
            Ok(key.to_string())
        })
    }

    fn copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, to) = (id(key)?, optional(folder)?);
        self.run(move |c| async move {
            let mut call = c.folders.copy_file_request();
            write_uuid(call.get().init_id(), Some(key));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(on_conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(folders::copy_file_results::Reader::get_file)
                .and_then(file_of)
                .map(|f| f.key)
                .map_err(|e| problem(&e))
        })
    }

    fn trash(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.trash_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    fn delete_folder(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.delete_folder_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    fn restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.restore_request();
            write_uuid(call.get().init_id(), Some(key));
            call.get().set_on_conflict(on_conflict(rule));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(key.to_string())
        })
    }

    fn restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, to) = (id(key)?, optional(folder)?);
        self.run(move |c| async move {
            let mut call = c.folders.restore_to_request();
            write_uuid(call.get().init_id(), Some(key));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(on_conflict(rule));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(key.to_string())
        })
    }

    fn purge(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.purge_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    fn trash_count(&self) -> Result<TrashCount, FileError> {
        self.run(move |c| async move {
            let (mut files, mut cursor) = (0usize, Vec::<u8>::new());
            loop {
                let mut call = c.search.find_request();
                call.get().init_filter().set_trashed(true);
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(search::find_results::Reader::get_page)
                    .map_err(|e| problem(&e))?;
                files += page.get_items().map_err(|e| problem(&e))?.len() as usize;
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    // Marked files are not told apart yet (ADR 0029): none is counted as marked.
                    return Ok(TrashCount::new(files, 0));
                }
                cursor = next.to_vec();
            }
        })
    }

    fn empty_trash(&self) -> Result<EmptyReport, FileError> {
        self.run(move |c| async move {
            let answer = c
                .folders
                .empty_trash_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(folders::empty_trash_results::Reader::get_report)
                .map_err(|e| problem(&e))?;
            let mut failed: Vec<String> = Vec::new();
            for u in report.get_failed().map_err(|e| problem(&e))? {
                if let Some(u) = read_uuid(u) {
                    failed.push(u.to_string());
                }
            }
            if report.get_failed_missing() > 0 {
                failed.push(format!("{} more", report.get_failed_missing()));
            }
            Ok(EmptyReport::new(report.get_purged() as usize, failed))
        })
    }

    fn rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        let folder = optional(folder)?;
        self.run(move |c| async move {
            let mut call = c.folders.rescan_request();
            write_uuid(call.get().init_folder(), folder);
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(folders::rescan_results::Reader::get_report)
                .map_err(|e| problem(&e))?;
            let mut summary = RescanSummary::default();
            for item in report.get_added().map_err(|e| problem(&e))? {
                summary
                    .added
                    .push(entry_of(item).map_err(|e| problem(&e))?.name);
            }
            for f in report.get_updated().map_err(|e| problem(&e))? {
                summary
                    .updated
                    .push(file_of(f).map_err(|e| problem(&e))?.name);
            }
            for f in report.get_missing().map_err(|e| problem(&e))? {
                summary
                    .missing
                    .push(file_of(f).map_err(|e| problem(&e))?.name);
            }
            for s in report.get_skipped().map_err(|e| problem(&e))? {
                summary.skipped.push(
                    s.map_err(|e| problem(&e))?
                        .to_string()
                        .map_err(|e| FileError::failed(e.to_string()))?,
                );
            }
            Ok(summary)
        })
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
