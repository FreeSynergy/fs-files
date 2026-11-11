//! The files server as a `FileSource` of the file workbench (ADR 0066: the program only wires).
//!
//! Pattern: Adapter — `files.capnp` behind the `FileSource` trait of `fs-components-workbenches`; the keys the
//! workbench holds are the bare ids of the records. Every call is a job of the worker of [`FilesRemote`] with a
//! limit, so a server that does not answer ends in a visible error, never in a window that waits for ever. The trait
//! is synchronous for now (the workbench runs it on the interface thread); the non-blocking `Session::submit` of
//! fs-components v0.0.10 replaces that, the adapter stays.

use std::sync::Arc;

use fs_admin_frontend::{BackendError, Values};
use fs_components_workbenches::file_manager::{
    EmptyReport, EntryKind, EntryPage, EntryRef, FileCapabilities, FileEntry, FileError,
    FileProblem, FileSource, FolderInfo, ListRequest, NameRule, PageMark, PendingUpload,
    PreviewBytes, RescanSummary, Scope, SortKey, TrashCount,
};
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::{OnConflict, SortBy};
use fs_interfaces::rpc::write_uuid;
use fs_types::TypeProblem;
use uuid::Uuid;

use crate::listing::{fill, find_all};
use crate::remote::{Clients, FilesRemote, problem};
use crate::values::{Found, entry_record, file_record, folder_record};

/// The files server as the source of the workbench.
pub struct RemoteFileSource(pub Arc<FilesRemote>);

/// The text of a time for the list: day and minute, in UTC.
fn shown(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339).map_or_else(
        |_| rfc3339.to_owned(),
        |t| t.format("%Y-%m-%d %H:%M").to_string(),
    )
}

/// An entry of the workbench from a record of the server.
fn entry_of(found: Found) -> FileEntry {
    let (record, folder) = match found {
        Found::File(r) => (r, false),
        Found::Folder(r) => (r, true),
    };
    let one = |k: &str| record.values.get(k).and_then(|v| v.first()).cloned();
    let key = bare(&record.id);
    let name = one("name").unwrap_or_default();
    let mut entry = if folder {
        FileEntry::folder(key, name)
    } else {
        FileEntry::file(
            key,
            name,
            one("size").and_then(|s| s.parse().ok()).unwrap_or(0),
        )
    };
    if !folder {
        entry.media_type = one("mediaType").unwrap_or_default();
        entry.trashed = one("state").as_deref() == Some("trashed");
        entry.sensitive = record.values.get("sensitive").cloned().unwrap_or_default();
    }
    entry.folder = one(if folder { "parent" } else { "folder" }).map(|f| bare(&f));
    entry.modified = one("updated").map(|t| shown(&t));
    entry
}

/// The bare id of `urn:uuid:<id>`.
fn bare(id: &str) -> String {
    id.strip_prefix("urn:uuid:").unwrap_or(id).to_owned()
}

fn id_of(key: &str) -> Result<Uuid, FileError> {
    Uuid::parse_str(key).map_err(|_| FileError::new(FileProblem::NotFound, "this is not a known id"))
}

fn folder_id(key: Option<&str>) -> Result<Option<Uuid>, FileError> {
    key.map(id_of).transpose()
}

fn conflict(rule: NameRule) -> OnConflict {
    match rule {
        NameRule::KeepBoth => OnConflict::KeepBoth,
        _ => OnConflict::Refuse,
    }
}

/// What the server said, as the workbench understands it.
fn error_of(error: BackendError) -> FileError {
    match error {
        BackendError::NotFound => FileError::new(FileProblem::NotFound, "it is not there any more"),
        BackendError::Rejected(issues) => {
            let kind = issues.first().and_then(|i| match &i.problem {
                fs_types::RecordProblem::Value(p) => Some(p.clone()),
                _ => None,
            });
            match kind {
                Some(TypeProblem::NotInList) => {
                    FileError::new(FileProblem::NameConflict, "this name is taken")
                }
                Some(TypeProblem::Empty | TypeProblem::TooLong | TypeProblem::Character) => {
                    FileError::new(FileProblem::InvalidName, "this name cannot be used")
                }
                _ => FileError::failed("the server did not accept this"),
            }
        }
        BackendError::Unavailable(text) => match text.strip_prefix("unsupported:") {
            Some(name) => FileError::unsupported(format!("not possible yet: {name}")),
            None => FileError::failed(text),
        },
        BackendError::Handshake(text) => FileError::failed(text),
    }
}

/// A time as the server wants it: RFC 3339, or a bare day (midnight UTC).
fn instant(text: &str) -> String {
    let text = text.trim();
    if chrono::DateTime::parse_from_rfc3339(text).is_ok() {
        text.to_owned()
    } else {
        format!("{text}T00:00:00Z")
    }
}

/// The query of `find` for what the person asked.
fn query_of(request: &ListRequest) -> Values {
    let mut q = Values::new();
    let mut put = |k: &str, v: String| {
        if !v.is_empty() {
            q.insert(k.to_owned(), vec![v]);
        }
    };
    put("name", request.term.clone());
    match &request.scope {
        Scope::Folder(Some(f)) => put("folder", f.clone()),
        Scope::Trash => put("state", "trashed".into()),
        _ => {}
    }
    let f = &request.filter;
    put("minSize", f.min_size.map(|n| n.to_string()).unwrap_or_default());
    put("maxSize", f.max_size.map(|n| n.to_string()).unwrap_or_default());
    put("modifiedAfter", f.modified_from.as_deref().map(instant).unwrap_or_default());
    put("modifiedBefore", f.modified_before.as_deref().map(instant).unwrap_or_default());
    q
}

fn sort_by(key: SortKey) -> SortBy {
    match key {
        SortKey::Modified => SortBy::Modified,
        SortKey::Size => SortBy::Size,
        SortKey::Type => SortBy::MediaType,
        _ => SortBy::Name,
    }
}

/// One page of the list: `list` of one folder when nothing narrows it, else `find`.
async fn page_of(
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
        call.send().promise.await.map_err(|e| problem(&e))?.get().and_then(|r| r.get_page().and_then(|p| read_page(p)))
    } else {
        let mut call = c.search.find_request();
        fill(call.get().init_filter(), &query_of(request), None)?;
        call.get().init_cursor().set_opaque(&cursor);
        {
            let mut s = call.get().init_sort();
            s.set_by(by);
            s.set_descending(descending);
        }
        call.send().promise.await.map_err(|e| problem(&e))?.get().and_then(|r| r.get_page().and_then(|p| read_page(p)))
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

/// The `file:` link of a path, as the transfer commands read it.
fn link_of(path: &str) -> String {
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn new_name(file: fs_interfaces::files_capnp::file::Reader<'_>) -> Result<String, BackendError> {
    let record = file_record(file).map_err(|e| problem(&e))?;
    Ok(record
        .values
        .get("name")
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default())
}

impl RemoteFileSource {
    fn within<T, F, Fut>(&self, command: &str, make: F) -> Result<T, FileError>
    where
        T: Send + 'static,
        F: FnOnce(std::rc::Rc<Clients>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, BackendError>> + 'static,
    {
        self.0
            .run_within(self.0.limit_of(command), make)
            .map_err(error_of)
    }

    fn run<T, F, Fut>(&self, make: F) -> Result<T, FileError>
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
        let cursor = after
            .and_then(|m| m.0.downcast_ref::<Vec<u8>>().cloned())
            .unwrap_or_default();
        let request = request.clone();
        let (entries, next) = self.run(move |c| async move { page_of(&c, &request, cursor).await })?;
        let next = (!next.is_empty()).then(|| PageMark(Arc::new(next)));
        Ok(EntryPage::new(entries, next))
    }

    fn entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
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

    fn folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
        let found = self.entry(&EntryRef {
            key: key.to_owned(),
            kind: EntryKind::Folder,
        })?;
        Ok(found.map(|e| FolderInfo::new(e.key, e.name, e.folder)))
    }

    fn subfolders(
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

    fn preview(&self, key: &str, limit: usize) -> Result<PreviewBytes, FileError> {
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
                read.get().set_length(u32::try_from(want).unwrap_or(u32::MAX));
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

    fn create_folder(&self, parent: Option<&str>, name: &str) -> Result<FolderInfo, FileError> {
        let (parent, name) = (folder_id(parent)?, name.to_owned());
        self.run(move |c| async move {
            let mut call = c.folders.create_folder_request();
            write_uuid(call.get().init_parent(), parent);
            call.get().set_name(name.as_str());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let f = answer
                .get()
                .and_then(|r| r.get_folder())
                .map_err(|e| problem(&e))?;
            let e = entry_of(Found::Folder(folder_record(f).map_err(|e| problem(&e))?));
            Ok(FolderInfo::new(e.key, e.name, e.folder))
        })
    }

    fn rename(&self, entry: &EntryRef, name: &str, rule: NameRule) -> Result<String, FileError> {
        let (id, name, folder) = (id_of(&entry.key)?, name.to_owned(), entry.kind == EntryKind::Folder);
        self.run(move |c| async move {
            if folder {
                let mut call = c.folders.rename_folder_request();
                write_uuid(call.get().init_id(), Some(id));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let f = answer.get().and_then(|r| r.get_folder()).map_err(|e| problem(&e))?;
                Ok(text_name(&entry_of(Found::Folder(folder_record(f).map_err(|e| problem(&e))?))))
            } else {
                let mut call = c.folders.rename_file_request();
                write_uuid(call.get().init_id(), Some(id));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
            }
        })
    }

    fn move_to(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to, is_folder) = (id_of(&entry.key)?, folder_id(folder)?, entry.kind == EntryKind::Folder);
        self.within("move", move |c| async move {
            if is_folder {
                let mut call = c.folders.move_folder_request();
                write_uuid(call.get().init_id(), Some(id));
                write_uuid(call.get().init_parent(), to);
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let f = answer.get().and_then(|r| r.get_folder()).map_err(|e| problem(&e))?;
                Ok(text_name(&entry_of(Found::Folder(folder_record(f).map_err(|e| problem(&e))?))))
            } else {
                let mut call = c.folders.move_file_request();
                write_uuid(call.get().init_id(), Some(id));
                write_uuid(call.get().init_folder(), to);
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
            }
        })
    }

    fn copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to) = (id_of(key)?, folder_id(folder)?);
        self.within("copy", move |c| async move {
            let mut call = c.folders.copy_file_request();
            write_uuid(call.get().init_id(), Some(id));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
        })
    }

    fn trash(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.trash_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    fn delete_folder(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.delete_folder_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    fn restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        let id = id_of(key)?;
        self.within("restore", move |c| async move {
            let mut call = c.folders.restore_request();
            write_uuid(call.get().init_id(), Some(id));
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
        })
    }

    fn restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to) = (id_of(key)?, folder_id(folder)?);
        self.within("restoreTo", move |c| async move {
            let mut call = c.folders.restore_to_request();
            write_uuid(call.get().init_id(), Some(id));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
        })
    }

    fn purge(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.within("purge", move |c| async move {
            let mut call = c.folders.purge_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    fn trash_count(&self) -> Result<TrashCount, FileError> {
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

    fn empty_trash(&self) -> Result<EmptyReport, FileError> {
        self.within("emptyTrash", move |c| async move {
            let answer = c
                .folders
                .empty_trash_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(|r| r.get_report())
                .map_err(|e| problem(&e))?;
            let failed = report.get_failed().map_err(|e| problem(&e))?;
            let mut names: Vec<String> = failed
                .iter()
                .filter_map(|u| fs_interfaces::rpc::read_uuid(u))
                .map(|u| u.to_string())
                .collect();
            names.extend((0..report.get_failed_missing()).map(|_| String::from("?")));
            Ok(EmptyReport::new(report.get_purged() as usize, names))
        })
    }

    fn rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        let folder = folder_id(folder)?;
        self.within("rescan", move |c| async move {
            let mut call = c.folders.rescan_request();
            write_uuid(call.get().init_folder(), folder);
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(|r| r.get_report())
                .map_err(|e| problem(&e))?;
            let mut out = RescanSummary::default();
            for e in report.get_added().map_err(|e| problem(&e))? {
                out.added
                    .push(text_name(&entry_of(entry_record(e).map_err(|e| problem(&e))?)));
            }
            for f in report.get_updated().map_err(|e| problem(&e))? {
                out.updated.push(new_name(f)?);
            }
            for f in report.get_missing().map_err(|e| problem(&e))? {
                out.missing.push(new_name(f)?);
            }
            for s in report.get_skipped().map_err(|e| problem(&e))? {
                out.skipped.push(
                    s.map_err(|e| problem(&e))?
                        .to_string()
                        .map_err(|e| problem(&e.into()))?,
                );
            }
            Ok(out)
        })
    }

    fn upload(
        &self,
        path: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let mut input = Values::from([("source".to_owned(), vec![link_of(path)])]);
        if let Some(f) = folder {
            input.insert("folder".into(), vec![f.to_owned()]);
        }
        if rule == NameRule::KeepBoth {
            input.insert("onConflict".into(), vec!["keep-both".into()]);
        }
        self.within("upload", move |c| async move {
            let out = crate::transfer::upload(&c.transfer, &input).await?;
            let id = out.id.unwrap_or_default();
            // the name the file has now
            let mut call = c.folders.get_request();
            write_uuid(call.get().init_id(), Uuid::parse_str(&bare(&id)).ok());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(answer.get().and_then(|r| r.get_file()).map_err(|e| problem(&e))?)
        })
    }

    fn pending_uploads(&self) -> Result<Vec<PendingUpload>, FileError> {
        self.run(move |c| async move {
            let out = crate::uploads::pending(&c.transfer).await?;
            let col = |k: &str| out.values.get(k).cloned().unwrap_or_default();
            let (ids, names, got) = (col("uploadId"), col("name"), col("received"));
            Ok(ids
                .into_iter()
                .zip(names)
                .zip(got)
                .map(|((id, name), n)| PendingUpload::new(id, name, n.parse().unwrap_or(0)))
                .collect())
        })
    }

    fn resume_upload(&self, _id: &str, _path: &str) -> Result<String, FileError> {
        Err(FileError::unsupported(
            "going on with an unfinished upload is not possible yet",
        ))
    }

    fn drop_upload(&self, id: &str) -> Result<(), FileError> {
        let input = Values::from([("uploadId".to_owned(), vec![id.to_owned()])]);
        self.run(move |c| async move {
            crate::uploads::abort(&c.transfer, &input).await?;
            Ok(())
        })
    }

    fn download(&self, key: &str, path: &str, _release_marked: bool) -> Result<String, FileError> {
        let id = id_of(key)?;
        let input = Values::from([("target".to_owned(), vec![link_of(path)])]);
        let target = path.to_owned();
        self.within("download", move |c| async move {
            crate::transfer::download(&c.transfer, id, &input).await?;
            Ok(target)
        })
    }
}

fn text_name(entry: &FileEntry) -> String {
    entry.name.clone()
}
