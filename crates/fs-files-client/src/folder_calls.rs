//! The calls of `Folders`, one function each, on the connection's thread.
//!
//! Pattern: Adapter — one request in, one reply read back as values of the core. A page is one call;
//! the cursor in and out is `cursor.rs`.

use fs_files_core::{
    Cursor, File, Folder, OnConflict, Page, RescanReport, Result, Sort, TrashReport,
};
use fs_interfaces::files_capnp::folders::Client;
use fs_interfaces::rpc::write_object_id;
use fs_objects::ObjectId;

use crate::problems::error_of;
use crate::read::{self, unreadable};
use crate::{cursor, write};

/// `list`.
pub(crate) async fn list(
    c: &Client,
    folder: Option<ObjectId>,
    at: Option<Cursor>,
    sort: Sort,
) -> Result<Page> {
    let mut req = c.list_request();
    write::top(req.get().init_folder(), folder);
    req.get().init_cursor().set_opaque(&cursor::write(at));
    write::order(req.get().init_sort(), sort);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    page(answer.get().and_then(|r| r.get_page()))
}

/// A page of entries and the cursor of the next one.
pub(crate) fn page(
    page: capnp::Result<fs_interfaces::files_capnp::entry_page::Reader<'_>>,
) -> Result<Page> {
    let page = page.map_err(unreadable)?;
    let items = page
        .get_items()
        .map_err(unreadable)?
        .iter()
        .map(read::entry)
        .collect::<Result<_>>()?;
    let next = page
        .get_next()
        .and_then(|n| n.get_opaque())
        .map_err(unreadable)?;
    Ok(Page {
        items,
        next: cursor::read(next)?,
    })
}

/// `get`: `None` when the server says `found = false`.
pub(crate) async fn get(c: &Client, id: ObjectId) -> Result<Option<File>> {
    let mut req = c.get_request();
    write_object_id(req.get().init_id(), id);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    let r = answer.get().map_err(unreadable)?;
    if !r.get_found() {
        return Ok(None);
    }
    read::file(r.get_file().map_err(unreadable)?).map(Some)
}

/// `getFolder`: `None` when the server says `found = false`.
pub(crate) async fn get_folder(c: &Client, id: ObjectId) -> Result<Option<Folder>> {
    let mut req = c.get_folder_request();
    write_object_id(req.get().init_id(), id);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    let r = answer.get().map_err(unreadable)?;
    if !r.get_found() {
        return Ok(None);
    }
    read::folder(r.get_folder().map_err(unreadable)?).map(Some)
}

/// `createFolder`.
pub(crate) async fn create_folder(
    c: &Client,
    parent: Option<ObjectId>,
    name: String,
) -> Result<Folder> {
    let mut req = c.create_folder_request();
    write::top(req.get().init_parent(), parent);
    req.get().set_name(name.as_str());
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    read::folder(
        answer
            .get()
            .and_then(|r| r.get_folder())
            .map_err(unreadable)?,
    )
}

/// `renameFolder`.
pub(crate) async fn rename_folder(
    c: &Client,
    id: ObjectId,
    name: String,
    on: OnConflict,
) -> Result<Folder> {
    let mut req = c.rename_folder_request();
    write_object_id(req.get().init_id(), id);
    req.get().set_name(name.as_str());
    req.get().set_on_conflict(write::conflict(on));
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    read::folder(
        answer
            .get()
            .and_then(|r| r.get_folder())
            .map_err(unreadable)?,
    )
}

/// `moveFolder`.
pub(crate) async fn move_folder(
    c: &Client,
    id: ObjectId,
    parent: Option<ObjectId>,
    on: OnConflict,
) -> Result<Folder> {
    let mut req = c.move_folder_request();
    write_object_id(req.get().init_id(), id);
    write::top(req.get().init_parent(), parent);
    req.get().set_on_conflict(write::conflict(on));
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    read::folder(
        answer
            .get()
            .and_then(|r| r.get_folder())
            .map_err(unreadable)?,
    )
}

/// `deleteFolder`.
pub(crate) async fn delete_folder(c: &Client, id: ObjectId) -> Result<()> {
    let mut req = c.delete_folder_request();
    write_object_id(req.get().init_id(), id);
    req.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(())
}

/// `rescan`.
pub(crate) async fn rescan(c: &Client, folder: Option<ObjectId>) -> Result<RescanReport> {
    let mut req = c.rescan_request();
    write::top(req.get().init_folder(), folder);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    read::rescan(
        answer
            .get()
            .and_then(|r| r.get_report())
            .map_err(unreadable)?,
    )
}

/// `emptyTrash`.
pub(crate) async fn empty_trash(c: &Client) -> Result<TrashReport> {
    let answer = c
        .empty_trash_request()
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e))?;
    read::trash(
        answer
            .get()
            .and_then(|r| r.get_report())
            .map_err(unreadable)?,
    )
}
