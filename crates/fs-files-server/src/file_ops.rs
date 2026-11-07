//! The file half of `Folders`: move, copy, rename, classify, resolve.
//!
//! Pattern: Adapter — wire request in, one `Files` call, wire response out; writes run to their end
//! (`finish`). A taken name is decided by the rule the caller sent (`onConflict`); there is no
//! replace, with or without a flag.

use std::sync::Arc;

use fs_files_core::Files;
use fs_interfaces::files_capnp::folders as f;
use fs_interfaces::rpc::{invalid_request, read_object_id};

use crate::errors::{finish, rpc_error};
use crate::wire::entry::write_file;
use crate::wire::ids::{conflict, optional};
use crate::wire::text::{self, MAX_CATEGORIES, MAX_SHORT};

type Done = capnp::Result<()>;

pub(crate) async fn move_to(
    files: &Arc<dyn Files>,
    p: f::MoveFileParams,
    mut r: f::MoveFileResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let folder = optional(p.get_folder()?)?;
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let file = finish(async move { files.move_file_as(id, folder, on).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn copy(
    files: &Arc<dyn Files>,
    p: f::CopyFileParams,
    mut r: f::CopyFileResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let folder = optional(p.get_folder()?)?;
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let file = finish(async move { files.copy_file_as(id, folder, on).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn rename(
    files: &Arc<dyn Files>,
    p: f::RenameFileParams,
    mut r: f::RenameFileResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let name = text::name(p.get_name()?)?.to_string();
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let file = finish(async move { files.rename_file_as(id, &name, on).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn classify(
    files: &Arc<dyn Files>,
    p: f::ClassifyParams,
    mut r: f::ClassifyResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let mut total = 0;
    let mut categories = Vec::new();
    for c in p.get_categories()? {
        let c = text::limited(c?, "a category", MAX_SHORT)?;
        total += c.len();
        if total > MAX_CATEGORIES {
            return Err(invalid_request(format!(
                "the categories together have more than {MAX_CATEGORIES} bytes"
            )));
        }
        categories.push(c.to_string());
    }
    let files = files.clone();
    let file = finish(async move { files.classify(id, categories).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn get(files: &Arc<dyn Files>, p: f::GetParams, mut r: f::GetResults) -> Done {
    let id = read_object_id(p.get()?.get_id()?)?;
    let found = files.get(id).await.map_err(rpc_error)?;
    let mut out = r.get();
    out.set_found(found.is_some());
    if let Some(file) = found {
        write_file(out.init_file(), &file);
    }
    Ok(())
}
