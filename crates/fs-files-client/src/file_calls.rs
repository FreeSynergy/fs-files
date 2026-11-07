//! The calls of `Folders` and `Search` that are about one file.
//!
//! Pattern: Adapter — one request in, one reply read back as a `File`. Split from `folder_calls.rs` by
//! the thing they act on.

use fs_files_core::{Cursor, File, FileFilter, OnConflict, Page, Result, Sort};
use fs_interfaces::files_capnp::{folders, search};
use fs_interfaces::rpc::write_object_id;
use fs_objects::ObjectId;

use crate::folder_calls::page;
use crate::problems::error_of;
use crate::read::{self, unreadable};
use crate::{cursor, write};

/// Which of the file calls with an id, a folder or a name and a rule.
pub(crate) enum Change {
    /// `moveFile`.
    Move(Option<ObjectId>),
    /// `copyFile`.
    Copy(Option<ObjectId>),
    /// `restoreTo`.
    RestoreTo(Option<ObjectId>),
    /// `renameFile`.
    Rename(String),
    /// `restore`.
    Restore,
}

macro_rules! file_call {
    ($c:expr, $make:ident, $id:expr, $on:expr, |$req:ident| $set:expr) => {{
        let mut $req = $c.$make();
        write_object_id($req.get().init_id(), $id);
        $req.get().set_on_conflict(write::conflict($on));
        $set;
        let answer = $req.send().promise.await.map_err(|e| error_of(&e))?;
        read::file(
            answer
                .get()
                .and_then(|r| r.get_file())
                .map_err(unreadable)?,
        )
    }};
}

/// A change to one file, with the rule for a taken name.
pub(crate) async fn change(
    c: &folders::Client,
    id: ObjectId,
    change: Change,
    on: OnConflict,
) -> Result<File> {
    match change {
        Change::Move(to) => file_call!(c, move_file_request, id, on, |r| write::top(
            r.get().init_folder(),
            to
        )),
        Change::Copy(to) => file_call!(c, copy_file_request, id, on, |r| write::top(
            r.get().init_folder(),
            to
        )),
        Change::RestoreTo(to) => file_call!(c, restore_to_request, id, on, |r| write::top(
            r.get().init_folder(),
            to
        )),
        Change::Rename(name) => file_call!(c, rename_file_request, id, on, |r| r
            .get()
            .set_name(name.as_str())),
        Change::Restore => file_call!(c, restore_request, id, on, |_r| {}),
    }
}

/// `trash`.
pub(crate) async fn trash(c: &folders::Client, id: ObjectId) -> Result<()> {
    let mut req = c.trash_request();
    write_object_id(req.get().init_id(), id);
    req.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(())
}

/// `purge`.
pub(crate) async fn purge(c: &folders::Client, id: ObjectId) -> Result<()> {
    let mut req = c.purge_request();
    write_object_id(req.get().init_id(), id);
    req.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(())
}

/// `find`.
pub(crate) async fn find(
    c: &search::Client,
    filter: FileFilter,
    at: Option<Cursor>,
    sort: Sort,
) -> Result<Page> {
    let mut req = c.find_request();
    write::filter(req.get().init_filter(), &filter);
    req.get().init_cursor().set_opaque(&cursor::write(at));
    write::order(req.get().init_sort(), sort);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    page(answer.get().and_then(|r| r.get_page()))
}
