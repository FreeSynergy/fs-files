//! The `Backend` of the data type `Folder` (ADR 0066): the list is a walk over the folder tree.
//!
//! Pattern: Adapter (+ Command dispatch by name) — `files.capnp` `Folders` behind the `Backend` trait. The contract
//! lists the children of one folder, so `find` walks from the top (breadth first, bounded by [`MAX_RECORDS`]).

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_files_types::is_supported;
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::folders;
use fs_interfaces::rpc::write_uuid;
use uuid::Uuid;

use crate::listing::MAX_RECORDS;
use crate::remote::{FilesRemote, problem, unsupported};
use crate::values::{Found, entry_record, folder_record, id_of, need_id, on_conflict, text};

/// The folders.
pub struct FolderBackend(pub Arc<FilesRemote>);

/// The folders directly below `parent` (none = the top).
async fn children(f: &folders::Client, parent: Option<Uuid>) -> Result<Vec<Record>, BackendError> {
    let mut out = Vec::new();
    let mut cursor: Vec<u8> = Vec::new();
    loop {
        let mut call = f.list_request();
        write_uuid(call.get().init_folder(), parent);
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        let page = answer
            .get()
            .and_then(|r| r.get_page())
            .map_err(|e| problem(&e))?;
        for item in page.get_items().map_err(|e| problem(&e))? {
            if let Found::Folder(r) = entry_record(item).map_err(|e| problem(&e))? {
                out.push(r);
            }
        }
        let next = page
            .get_next()
            .and_then(common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| problem(&e))?;
        if next.is_empty() {
            return Ok(out);
        }
        cursor = next.to_vec();
    }
}

async fn walk(f: &folders::Client, name: Option<String>) -> Result<Vec<Record>, BackendError> {
    let mut all = Vec::new();
    let mut queue = std::collections::VecDeque::from([None]);
    while let Some(parent) = queue.pop_front() {
        for r in children(f, parent).await? {
            if let Ok(id) = r.id.parse::<Uuid>() {
                queue.push_back(Some(id));
            }
            all.push(r);
            if all.len() >= MAX_RECORDS {
                queue.clear();
                break;
            }
        }
    }
    if let Some(word) = name {
        let word = word.to_lowercase();
        all.retain(|r| {
            r.values
                .get("name")
                .and_then(|v| v.first())
                .is_some_and(|n| n.to_lowercase().contains(&word))
        });
    }
    Ok(all)
}

impl Backend for FolderBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        let name = text(query, "name").map(str::to_owned);
        self.0
            .run(move |c| async move { walk(&c.folders, name).await })
    }

    fn run(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        if !is_supported("Folder", name) {
            return Err(unsupported(name));
        }
        let (name, input) = (name.to_owned(), input.clone());
        self.0.run(move |c| async move {
            let f = &c.folders;
            match name.as_str() {
                "create" => create(f, &input).await,
                "rename" => rename(f, &input).await,
                "move" => relocate(f, &input).await,
                "delete" => delete(f, &input).await,
                "resolve" => resolve(f, &input).await,
                "rescan" => rescan(f, &input).await,
                _ => Err(unsupported(&name)),
            }
        })
    }
}

fn done(id: impl ToString) -> Outcome {
    Outcome {
        id: Some(id.to_string()),
        values: Values::new(),
    }
}

async fn create(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let mut call = f.create_folder_request();
    write_uuid(call.get().init_parent(), id_of(input, "parent")?);
    call.get().set_name(text(input, "name").unwrap_or_default());
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let folder = answer
        .get()
        .and_then(|r| r.get_folder())
        .map_err(|e| problem(&e))?;
    let id = folder_record(folder).map_err(|e| problem(&e))?.id;
    let mut out = done(&id);
    out.values.insert("created".into(), vec![id]);
    Ok(out)
}

async fn rename(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let mut call = f.rename_folder_request();
    write_uuid(call.get().init_id(), Some(need_id(input, "folder")?));
    call.get().set_name(text(input, "name").unwrap_or_default());
    call.get().set_on_conflict(on_conflict(input));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let folder = answer
        .get()
        .and_then(|r| r.get_folder())
        .map_err(|e| problem(&e))?;
    Ok(done(folder_record(folder).map_err(|e| problem(&e))?.id))
}

async fn relocate(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let mut call = f.move_folder_request();
    write_uuid(call.get().init_id(), Some(need_id(input, "folder")?));
    write_uuid(call.get().init_parent(), id_of(input, "parent")?);
    call.get().set_on_conflict(on_conflict(input));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let folder = answer
        .get()
        .and_then(|r| r.get_folder())
        .map_err(|e| problem(&e))?;
    Ok(done(folder_record(folder).map_err(|e| problem(&e))?.id))
}

async fn delete(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "folder")?;
    let mut call = f.delete_folder_request();
    write_uuid(call.get().init_id(), Some(id));
    call.send().promise.await.map_err(|e| problem(&e))?;
    Ok(done(id))
}

async fn resolve(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "folder")?;
    let mut call = f.get_folder_request();
    write_uuid(call.get().init_id(), Some(id));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    if answer.get().map_err(|e| problem(&e))?.get_found() {
        Ok(done(id))
    } else {
        Err(BackendError::NotFound)
    }
}

/// Takes over what lies on the disk below the folder; answers the names of what was added, updated, missing, skipped.
async fn rescan(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "folder")?;
    let mut call = f.rescan_request();
    write_uuid(call.get().init_folder(), Some(id));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let report = answer
        .get()
        .and_then(|r| r.get_report())
        .map_err(|e| problem(&e))?;
    let mut out = done(id);
    let names = |r: capnp::Result<Record>| r.ok().and_then(|r| r.values.get("name").cloned());
    let mut added = Vec::new();
    for e in report.get_added().map_err(|e| problem(&e))? {
        match entry_record(e).map_err(|e| problem(&e))? {
            Found::File(r) | Found::Folder(r) => added.extend(names(Ok(r)).unwrap_or_default()),
        }
    }
    let list = |files: capnp::struct_list::Reader<'_, fs_interfaces::files_capnp::file::Owned>| {
        files
            .iter()
            .filter_map(|f| names(crate::values::file_record(f)))
            .flatten()
            .collect::<Vec<_>>()
    };
    let updated = list(report.get_updated().map_err(|e| problem(&e))?);
    let missing = list(report.get_missing().map_err(|e| problem(&e))?);
    let mut skipped = Vec::new();
    for s in report.get_skipped().map_err(|e| problem(&e))? {
        skipped.push(
            s.map_err(|e| problem(&e))?
                .to_string()
                .map_err(|e| problem(&e.into()))?,
        );
    }
    for (key, values) in [
        ("added", added),
        ("updated", updated),
        ("missing", missing),
        ("skipped", skipped),
    ] {
        out.values.insert(key.into(), values);
    }
    Ok(out)
}
