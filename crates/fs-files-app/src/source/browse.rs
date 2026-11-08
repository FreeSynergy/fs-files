//! Reading the tree: pages of a folder or a search, one entry, one folder, the subfolders.
//!
//! Pattern: Adapter — contract calls of `folders`/`search` turned into the workbench's entries and pages.

use fs_components_workbenches::file_manager::{
    EntryKind, EntryPage, EntryRef, FileEntry, FileError, FolderInfo, ListRequest, PageMark, Scope,
};
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::{folders, search};
use fs_interfaces::rpc::write_uuid;

use super::convert::{cursor_of, entry_of, file_of, folder_of, id, optional, page_of, sort_by};
use crate::remote::{FilesRemote, problem};

impl FilesRemote {
    #[allow(clippy::single_match_else)] // two ways to the same page: one folder, or a search
    pub(super) fn do_list(
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
                    let mut found = page_of(page.get_items().map_err(|e| problem(&e))?, next)
                        .map_err(|e| problem(&e))?;
                    // The contract cannot say "the top level only": an unset folder means anywhere. A
                    // filter without a search word at the top level keeps what lies at the top.
                    if term.is_empty() && matches!(scope, Scope::Folder(None)) {
                        found.entries.retain(|e| e.folder.is_none());
                    }
                    Ok(found)
                })
            }
        }
    }

    pub(super) fn do_entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
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

    pub(super) fn do_folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
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

    pub(super) fn do_subfolders(
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
}
