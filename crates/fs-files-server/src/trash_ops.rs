//! The trash half of `Folders`: trash, restore, restore to another folder, purge, empty the trash.
//!
//! Pattern: Adapter — wire request in, one `Files` call, wire response out; every one of these
//! writes, so every one runs to its end (`finish`). Purge and empty are for good: the bytes first,
//! then the record, as the core does it.

use std::sync::Arc;

use fs_files_core::Files;
use fs_interfaces::files_capnp::folders as f;
use fs_interfaces::rpc::read_object_id;

use crate::errors::finish;
use crate::wire::entry::write_file;
use crate::wire::ids::{conflict, optional};
use crate::wire::report::write_trash;

type Done = capnp::Result<()>;

pub(crate) async fn trash(files: &Arc<dyn Files>, p: f::TrashParams) -> Done {
    let id = read_object_id(p.get()?.get_id()?)?;
    let files = files.clone();
    finish(async move { files.trash(id).await }).await
}

pub(crate) async fn restore(
    files: &Arc<dyn Files>,
    p: f::RestoreParams,
    mut r: f::RestoreResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let file = finish(async move { files.restore_as(id, on).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn restore_to(
    files: &Arc<dyn Files>,
    p: f::RestoreToParams,
    mut r: f::RestoreToResults,
) -> Done {
    let p = p.get()?;
    let id = read_object_id(p.get_id()?)?;
    let folder = optional(p.get_folder()?)?;
    let on = conflict(p.get_on_conflict())?;
    let files = files.clone();
    let file = finish(async move { files.restore_to(id, folder, on).await }).await?;
    write_file(r.get().init_file(), &file);
    Ok(())
}

pub(crate) async fn purge(files: &Arc<dyn Files>, p: f::PurgeParams) -> Done {
    let id = read_object_id(p.get()?.get_id()?)?;
    let files = files.clone();
    finish(async move { files.purge(id).await }).await
}

pub(crate) async fn empty(files: &Arc<dyn Files>, mut r: f::EmptyTrashResults) -> Done {
    let files = files.clone();
    let report = finish(async move { files.empty_trash().await }).await?;
    write_trash(r.get().init_report(), &report)
}
