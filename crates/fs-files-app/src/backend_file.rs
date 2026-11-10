//! The `Backend` of the data type `File` (ADR 0066): the list is `find`, the commands are calls of the contract.
//!
//! Pattern: Adapter (+ Command dispatch by name) — `files.capnp` behind the `Backend` trait of the generated frontend.
//! A command the core does not have (`fs-files-types` table) is refused with `unsupported:<name>`; the frontend never
//! sends one, because its control is locked until the description says otherwise.

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_files_types::is_supported;
use fs_interfaces::files_capnp::{folders, transfer};
use fs_interfaces::rpc::write_uuid;

use crate::listing::find_all;
use crate::remote::{FilesRemote, problem, unsupported};
use crate::transfer::{download, upload};
use crate::values::{Found, file_record, id_of, need_id, on_conflict, text};

/// The files.
pub struct FileBackend(pub Arc<FilesRemote>);

fn done(id: impl ToString) -> Outcome {
    Outcome {
        id: Some(id.to_string()),
        values: Values::new(),
    }
}

impl Backend for FileBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        let query = query.clone();
        self.0.run(move |c| async move {
            Ok(find_all(&c, &query, None)
                .await?
                .into_iter()
                .filter_map(|f| match f {
                    Found::File(r) => Some(r),
                    Found::Folder(_) => None,
                })
                .collect())
        })
    }

    fn run(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        if !is_supported("File", name) {
            return Err(unsupported(name));
        }
        let (name, input) = (name.to_owned(), input.clone());
        self.0.run(move |c| async move {
            let f = &c.folders;
            match name.as_str() {
                "rename" => rename(f, &input).await,
                "move" => relocate(f, &input, false).await,
                "copy" => relocate(f, &input, true).await,
                "restoreTo" => restore(f, &input, true).await,
                "restore" => restore(f, &input, false).await,
                "classify" => classify(f, &input).await,
                "delete" | "purge" => remove(f, &input, name == "purge").await,
                "resolve" => resolve(f, &input).await,
                "emptyTrash" => empty_trash(f).await,
                "upload" => upload(&c.transfer, &input).await,
                "download" => download(&c.transfer, need_id(&input, "file")?, &input).await,
                "uploadList" => pending(&c.transfer).await,
                "uploadAbort" => abort(&c.transfer, &input).await,
                _ => Err(unsupported(&name)),
            }
        })
    }
}

async fn rename(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "file")?;
    let mut call = f.rename_file_request();
    write_uuid(call.get().init_id(), Some(id));
    call.get().set_name(text(input, "name").unwrap_or_default());
    call.get().set_on_conflict(on_conflict(input));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let file = answer
        .get()
        .and_then(|r| r.get_file())
        .map_err(|e| problem(&e))?;
    Ok(done(file_record(file).map_err(|e| problem(&e))?.id))
}

/// Move (`copy` false) or copy a file into a folder; no folder = the top level.
async fn relocate(
    f: &folders::Client,
    input: &Values,
    copy: bool,
) -> Result<Outcome, BackendError> {
    let (id, folder) = (need_id(input, "file")?, id_of(input, "folder")?);
    let file = if copy {
        let mut call = f.copy_file_request();
        write_uuid(call.get().init_id(), Some(id));
        write_uuid(call.get().init_folder(), folder);
        call.get().set_on_conflict(on_conflict(input));
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        file_record(
            answer
                .get()
                .and_then(|r| r.get_file())
                .map_err(|e| problem(&e))?,
        )
    } else {
        let mut call = f.move_file_request();
        write_uuid(call.get().init_id(), Some(id));
        write_uuid(call.get().init_folder(), folder);
        call.get().set_on_conflict(on_conflict(input));
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        file_record(
            answer
                .get()
                .and_then(|r| r.get_file())
                .map_err(|e| problem(&e))?,
        )
    }
    .map_err(|e| problem(&e))?;
    let mut out = done(&file.id);
    if copy {
        out.values.insert("copy".into(), vec![file.id]);
    }
    Ok(out)
}

/// Restore into the original folder, or into `folder` (`to`; none = the top level).
async fn restore(f: &folders::Client, input: &Values, to: bool) -> Result<Outcome, BackendError> {
    let id = need_id(input, "file")?;
    let file = if to {
        let mut call = f.restore_to_request();
        write_uuid(call.get().init_id(), Some(id));
        write_uuid(call.get().init_folder(), id_of(input, "folder")?);
        call.get().set_on_conflict(on_conflict(input));
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        file_record(
            answer
                .get()
                .and_then(|r| r.get_file())
                .map_err(|e| problem(&e))?,
        )
    } else {
        let mut call = f.restore_request();
        write_uuid(call.get().init_id(), Some(id));
        call.get().set_on_conflict(on_conflict(input));
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        file_record(
            answer
                .get()
                .and_then(|r| r.get_file())
                .map_err(|e| problem(&e))?,
        )
    }
    .map_err(|e| problem(&e))?;
    Ok(done(file.id))
}

/// The categories of the sensitive marking; an empty list clears it (the rule for changing).
async fn classify(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "file")?;
    let categories: Vec<String> = input
        .get("sensitive")
        .map(|v| v.iter().filter(|s| !s.trim().is_empty()).cloned().collect())
        .unwrap_or_default();
    let mut call = f.classify_request();
    write_uuid(call.get().init_id(), Some(id));
    let mut list = call
        .get()
        .init_categories(u32::try_from(categories.len()).unwrap_or(u32::MAX));
    for (i, c) in (0..).zip(&categories) {
        list.set(i, c.as_str());
    }
    call.send().promise.await.map_err(|e| problem(&e))?;
    Ok(done(id))
}

/// Trash (`for_good` false) or purge a file that is in the trash.
async fn remove(
    f: &folders::Client,
    input: &Values,
    for_good: bool,
) -> Result<Outcome, BackendError> {
    let id = need_id(input, "file")?;
    if for_good {
        let mut call = f.purge_request();
        write_uuid(call.get().init_id(), Some(id));
        call.send().promise.await.map_err(|e| problem(&e))?;
    } else {
        let mut call = f.trash_request();
        write_uuid(call.get().init_id(), Some(id));
        call.send().promise.await.map_err(|e| problem(&e))?;
    }
    Ok(done(id))
}

async fn resolve(f: &folders::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = need_id(input, "file")?;
    let mut call = f.get_request();
    write_uuid(call.get().init_id(), Some(id));
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let r = answer.get().map_err(|e| problem(&e))?;
    if r.get_found() {
        Ok(done(id))
    } else {
        Err(BackendError::NotFound)
    }
}

async fn empty_trash(f: &folders::Client) -> Result<Outcome, BackendError> {
    let answer = f
        .empty_trash_request()
        .send()
        .promise
        .await
        .map_err(|e| problem(&e))?;
    let report = answer
        .get()
        .and_then(|r| r.get_report())
        .map_err(|e| problem(&e))?;
    let mut out = Outcome::default();
    out.values
        .insert("purged".into(), vec![report.get_purged().to_string()]);
    Ok(out)
}

async fn pending(t: &transfer::Client) -> Result<Outcome, BackendError> {
    let mut call = t.pending_uploads_request();
    call.get().init_cursor().set_opaque(&[]);
    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
    let items = answer
        .get()
        .and_then(|r| r.get_items())
        .map_err(|e| problem(&e))?;
    let mut out = Outcome::default();
    for u in items {
        let received = u.get_received();
        let id = u
            .get_id()
            .map_err(|e| problem(&e))?
            .to_string()
            .map_err(|e| problem(&e.into()))?;
        let name = u
            .get_name()
            .map_err(|e| problem(&e))?
            .to_string()
            .map_err(|e| problem(&e.into()))?;
        out.values.entry("uploadId".into()).or_default().push(id);
        out.values.entry("name".into()).or_default().push(name);
        out.values
            .entry("received".into())
            .or_default()
            .push(received.to_string());
    }
    Ok(out)
}

async fn abort(t: &transfer::Client, input: &Values) -> Result<Outcome, BackendError> {
    let id = text(input, "uploadId").ok_or(BackendError::NotFound)?;
    let mut resume = t.resume_upload_request();
    resume.get().set_id(id);
    let answer = resume.send().promise.await.map_err(|e| problem(&e))?;
    let upload = answer
        .get()
        .and_then(|r| r.get_upload())
        .map_err(|e| problem(&e))?;
    upload
        .abort_request()
        .send()
        .promise
        .await
        .map_err(|e| problem(&e))?;
    Ok(Outcome::default())
}
