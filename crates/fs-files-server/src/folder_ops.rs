//! The folder half of `Folders`: list, create, rename, move, delete, resolve, rescan.
//!
//! Pattern: Adapter — wire request in, one `Files` call, wire response out. Every write goes through
//! `finish`, so it reaches its end when the client goes away. What the provider cannot do arrives as
//! its explicit `Unsupported` and leaves as a typed problem.

use std::sync::Arc;

use fs_files_core::Files;
use fs_interfaces::files_capnp::folders as f;
use fs_interfaces::rpc::read_object_id;

use crate::errors::{finish, rpc_error};
use crate::wire::entry::{write_folder, write_page};
use crate::wire::ids::{conflict, optional, sort};
use crate::wire::report::write_rescan;
use crate::wire::{cursor, text};

type Done = capnp::Result<()>;

pub(crate) async fn list(files: &Arc<dyn Files>, p: f::ListParams, mut r: f::ListResults) -> Done {
    let p = p.get()?;
    let at = cursor::read(p.get_cursor()?.get_opaque()?)?;
    let page = files
        .list_sorted(optional(p.get_folder()?)?, at, sort(p.get_sort()?)?)
        .await
        .map_err(rpc_error)?;
    write_page(r.get().init_page(), &page)
}

pub(crate) async fn create(
    files: &Arc<dyn Files>,
    p: f::CreateFolderParams,
    mut r: f::CreateFolderResults,
) -> Done {
    let p = p.get()?;
    let parent = optional(p.get_parent()?)?;
    let name = text::name(p.get_name()?)?.to_string();
    let files = files.clone();
    let folder = finish(async move { files.create_folder(parent, &name).await }).await?;
    write_folder(r.get().init_folder(), &folder);
    Ok(())
}

pub(crate) async fn rename(
    files: &Arc<dyn Files>,
    p: f::RenameFolderParams,
    mut r: f::RenameFolderResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let name = text::name(p.get_name()?)?.to_string();
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let folder = finish(async move { files.rename_folder_as(id, &name, on).await }).await?;
    write_folder(r.get().init_folder(), &folder);
    Ok(())
}

pub(crate) async fn move_to(
    files: &Arc<dyn Files>,
    p: f::MoveFolderParams,
    mut r: f::MoveFolderResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let parent = optional(p.get_parent()?)?;
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let folder = finish(async move { files.move_folder_as(id, parent, on).await }).await?;
    write_folder(r.get().init_folder(), &folder);
    Ok(())
}

pub(crate) async fn delete(files: &Arc<dyn Files>, p: f::DeleteFolderParams) -> Done {
    let id = read_object_id(p.get()?.get_id()?)?;
    let files = files.clone();
    finish(async move { files.delete_folder(id).await }).await
}

pub(crate) async fn get(
    files: &Arc<dyn Files>,
    p: f::GetFolderParams,
    mut r: f::GetFolderResults,
) -> Done {
    let id = read_object_id(p.get()?.get_id()?)?;
    let found = files.get_folder(id).await.map_err(rpc_error)?;
    let mut out = r.get();
    out.set_found(found.is_some());
    if let Some(folder) = found {
        write_folder(out.init_folder(), &folder);
    }
    Ok(())
}

pub(crate) async fn rescan(
    files: &Arc<dyn Files>,
    p: f::RescanParams,
    mut r: f::RescanResults,
) -> Done {
    let folder = optional(p.get()?.get_folder()?)?;
    let files = files.clone();
    let report = finish(async move { files.rescan(folder).await }).await?;
    write_rescan(r.get().init_report(), &report)
}
