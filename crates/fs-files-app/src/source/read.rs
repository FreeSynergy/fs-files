//! Part of the window source: reading.
//!
//! Pattern: Adapter — see `mod.rs`.

use std::sync::Arc;

use fs_admin_frontend::{BackendError, Values};
use fs_components_workbenches::file_manager::{
    EntryKind, EntryPage, EntryRef, FileEntry, FileError, FolderInfo, ListRequest, PageMark,
    PreviewBytes, Scope, TrashCount,
};
use fs_interfaces::common_capnp;
use fs_interfaces::rpc::write_uuid;
use uuid::Uuid;

use super::adapter::RemoteFileSource;
use super::convert::{entry_of, folder_id, id_of, query_of, sort_by};
use crate::listing::{fill, find_all};
use crate::remote::{Clients, problem};
use crate::values::{Found, entry_record, file_record, folder_record};

/// One page of the list: `list` of one folder when nothing narrows it, else `find`.
pub(super) async fn page_of(
    c: &Clients,
    request: &ListRequest,
    cursor: Vec<u8>,
) -> Result<(Vec<FileEntry>, Vec<u8>), BackendError> {
    let plain = matches!(request.scope, Scope::Folder(_))
        && request.term.is_empty()
        && !request.filter.is_active();
    let (by, descending) = (sort_by(request.sort.key), request.sort.descending);
    let answer = if plain {
        let Scope::Folder(folder) = &request.scope else {
            unreachable!("plain is only for a folder")
        };
        let folder = folder
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| BackendError::NotFound)?;
        let mut call = c.folders.list_request();
        write_uuid(call.get().init_folder(), folder);
        call.get().init_cursor().set_opaque(&cursor);
        {
            let mut s = call.get().init_sort();
            s.set_by(by);
            s.set_descending(descending);
        }
        call.send()
            .promise
            .await
            .map_err(|e| problem(&e))?
            .get()
            .and_then(|r| r.get_page().and_then(|p| read_page(p)))
    } else {
        let mut call = c.search.find_request();
        fill(call.get().init_filter(), &query_of(request), None)?;
        call.get().init_cursor().set_opaque(&cursor);
        {
            let mut s = call.get().init_sort();
            s.set_by(by);
            s.set_descending(descending);
        }
        call.send()
            .promise
            .await
            .map_err(|e| problem(&e))?
            .get()
            .and_then(|r| r.get_page().and_then(|p| read_page(p)))
    };
    answer.map_err(|e| problem(&e))
}

fn read_page(
    page: fs_interfaces::files_capnp::entry_page::Reader<'_>,
) -> capnp::Result<(Vec<FileEntry>, Vec<u8>)> {
    let mut out = Vec::new();
    for item in page.get_items()? {
        out.push(entry_of(entry_record(item)?));
    }
    let next = page
        .get_next()
        .and_then(common_capnp::cursor::Reader::get_opaque)?;
    Ok((out, next.to_vec()))
}

impl RemoteFileSource {
    pub(super) fn do_list(
        &self,
        request: &ListRequest,
        after: Option<&PageMark>,
    ) -> Result<EntryPage, FileError> {
        let cursor = after
            .and_then(|m| m.0.downcast_ref::<Vec<u8>>().cloned())
            .unwrap_or_default();
        let request = request.clone();
        let (entries, next) =
            self.run(move |c| async move { page_of(&c, &request, cursor).await })?;
        let next = (!next.is_empty()).then(|| PageMark(Arc::new(next)));
        Ok(EntryPage::new(entries, next))
    }

    pub(super) fn do_entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
        let id = id_of(&reference.key)?;
        let folder = reference.kind == EntryKind::Folder;
        self.run(move |c| async move {
            if folder {
                let mut call = c.folders.get_folder_request();
                write_uuid(call.get().init_id(), Some(id));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let r = answer.get().map_err(|e| problem(&e))?;
                if !r.get_found() {
                    return Ok(None);
                }
                let f = r.get_folder().map_err(|e| problem(&e))?;
                Ok(Some(entry_of(Found::Folder(
                    folder_record(f).map_err(|e| problem(&e))?,
                ))))
            } else {
                let mut call = c.folders.get_request();
                write_uuid(call.get().init_id(), Some(id));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let r = answer.get().map_err(|e| problem(&e))?;
                if !r.get_found() {
                    return Ok(None);
                }
                let f = r.get_file().map_err(|e| problem(&e))?;
                Ok(Some(entry_of(Found::File(
                    file_record(f).map_err(|e| problem(&e))?,
                ))))
            }
        })
    }

    pub(super) fn do_folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
        let found = self.do_entry(&EntryRef {
            key: key.to_owned(),
            kind: EntryKind::Folder,
        })?;
        Ok(found.map(|e| FolderInfo::new(e.key, e.name, e.folder)))
    }

    pub(super) fn do_subfolders(
        &self,
        parent: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<FolderInfo>, bool), FileError> {
        let parent = folder_id(parent)?;
        self.run(move |c| async move {
            let mut out = Vec::new();
            let mut cursor: Vec<u8> = Vec::new();
            loop {
                let mut call = c.folders.list_request();
                write_uuid(call.get().init_folder(), parent);
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(|r| r.get_page())
                    .map_err(|e| problem(&e))?;
                for item in page.get_items().map_err(|e| problem(&e))? {
                    if let Found::Folder(r) = entry_record(item).map_err(|e| problem(&e))? {
                        let e = entry_of(Found::Folder(r));
                        out.push(FolderInfo::new(e.key, e.name, e.folder));
                    }
                }
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() || out.len() > limit {
                    let more = out.len() > limit;
                    out.truncate(limit);
                    return Ok((out, more));
                }
                cursor = next.to_vec();
            }
        })
    }

    pub(super) fn do_preview(&self, key: &str, limit: usize) -> Result<PreviewBytes, FileError> {
        let id = id_of(key)?;
        self.within("download", move |c| async move {
            let mut get = c.folders.get_request();
            write_uuid(get.get().init_id(), Some(id));
            let answer = get.send().promise.await.map_err(|e| problem(&e))?;
            let total = answer
                .get()
                .and_then(|r| r.get_file())
                .map(|f| f.get_size())
                .map_err(|e| problem(&e))?;
            let mut open = c.transfer.open_request();
            write_uuid(open.get().init_id(), Some(id));
            let blob = open
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?
                .get()
                .and_then(|r| r.get_blob())
                .map_err(|e| problem(&e))?;
            let mut bytes = Vec::new();
            while bytes.len() < limit {
                let mut read = blob.read_request();
                read.get().set_offset(bytes.len() as u64);
                let want = (limit - bytes.len()).min(1 << 20);
                read.get()
                    .set_length(u32::try_from(want).unwrap_or(u32::MAX));
                let answer = read.send().promise.await.map_err(|e| problem(&e))?;
                let data = answer
                    .get()
                    .and_then(|r| r.get_data())
                    .map_err(|e| problem(&e))?;
                if data.is_empty() {
                    break;
                }
                bytes.extend_from_slice(data);
            }
            Ok(PreviewBytes::new(bytes, total))
        })
    }

    pub(super) fn do_trash_count(&self) -> Result<TrashCount, FileError> {
        self.run(move |c| async move {
            let query = Values::from([("state".to_owned(), vec!["trashed".to_owned()])]);
            let found = find_all(&c, &query, None).await?;
            let marked = found
                .iter()
                .filter(|f| matches!(f, Found::File(r) if r.values.contains_key("sensitive")))
                .count();
            Ok(TrashCount::new(found.len(), marked))
        })
    }
}
