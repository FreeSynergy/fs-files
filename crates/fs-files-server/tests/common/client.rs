//! What a test does as a consumer of the program: the capabilities and the small calls every test needs.
#![allow(dead_code)] // each test file uses a different subset

use super::seen::{Seen, read_entry, read_file};
use fs_interfaces::common_capnp::uuid as wire_uuid;
use fs_interfaces::files_capnp::{OnConflict, backends, files, folders, search, transfer};
use fs_interfaces::rpc::{read_envelope, read_uuid, write_uuid};
use fs_objects::Envelope;
use uuid::Uuid;

/// The four capabilities behind the root.
pub struct Caps {
    pub folders: folders::Client,
    pub transfer: transfer::Client,
    pub search: search::Client,
    pub backends: backends::Client,
}

pub async fn caps(root: &files::Client) -> Caps {
    Caps {
        folders: root
            .folders_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_folders()
            .unwrap(),
        transfer: root
            .transfer_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_transfer()
            .unwrap(),
        search: root
            .search_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_search()
            .unwrap(),
        backends: root
            .backends_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_backends()
            .unwrap(),
    }
}

pub fn set_id(out: wire_uuid::Builder<'_>, id: Uuid) {
    write_uuid(out, Some(id));
}

/// Create a folder.
pub async fn mkdir(c: &Caps, parent: Option<Uuid>, name: &str) -> capnp::Result<Uuid> {
    let mut call = c.folders.create_folder_request();
    write_uuid(call.get().init_parent(), parent);
    call.get().set_name(name);
    let answer = call.send().promise.await?;
    let folder = answer.get()?.get_folder()?;
    Ok(read_envelope(folder.get_envelope()?)?.id.as_uuid())
}

/// Upload `data` in one chunk.
pub async fn put_file(
    c: &Caps,
    folder: Option<Uuid>,
    name: &str,
    media_type: &str,
    data: &[u8],
    on: OnConflict,
) -> capnp::Result<Seen> {
    let mut start = c.transfer.start_upload_request();
    write_uuid(start.get().init_folder(), folder);
    start.get().set_name(name);
    start.get().set_media_type(media_type);
    start.get().set_on_conflict(on);
    let started = start.send().promise.await?;
    let upload = started.get()?.get_upload()?;
    let mut write = upload.write_request();
    write.get().set_offset(0);
    write.get().set_data(data);
    write.send().promise.await?;
    let done = upload.finish_request().send().promise.await?;
    Ok(read_file(done.get()?.get_file()?))
}

/// A file with text in it, refused names are errors.
pub async fn text_file(c: &Caps, folder: Option<Uuid>, name: &str, text: &str) -> Seen {
    put_file(
        c,
        folder,
        name,
        "text/plain",
        text.as_bytes(),
        OnConflict::Refuse,
    )
    .await
    .unwrap()
}

/// Every entry of a folder, over all pages, as (name, is a folder, id).
pub async fn list_all(c: &Caps, folder: Option<Uuid>) -> Vec<(String, bool, Uuid)> {
    let (mut out, mut cursor) = (vec![], vec![]);
    loop {
        let mut call = c.folders.list_request();
        write_uuid(call.get().init_folder(), folder);
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.unwrap();
        let page = answer.get().unwrap().get_page().unwrap();
        out.extend(page.get_items().unwrap().iter().map(read_entry));
        cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
        if cursor.is_empty() {
            return out;
        }
    }
}

pub fn names(entries: &[(String, bool, Uuid)]) -> Vec<&str> {
    entries.iter().map(|e| e.0.as_str()).collect()
}

/// All bytes of a file, read through its `Blob` in pieces of `piece`.
pub async fn download(c: &Caps, id: Uuid, piece: u32) -> capnp::Result<Vec<u8>> {
    let mut open = c.transfer.open_request();
    set_id(open.get().init_id(), id);
    let blob = open.send().promise.await?.get()?.get_blob()?;
    let mut out = vec![];
    loop {
        let mut read = blob.read_request();
        read.get().set_offset(out.len() as u64);
        read.get().set_length(piece);
        let answer = read.send().promise.await?;
        let data = answer.get()?.get_data()?;
        if data.is_empty() {
            return Ok(out);
        }
        out.extend_from_slice(data);
    }
}

/// The problem an error carries: its type and status must be exactly these.
pub fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = fs_interfaces::rpc::Problem::from_error(error).expect("a typed problem");
    assert_eq!(
        (p.problem_type.as_str(), p.status),
        (problem_type, status),
        "{}",
        p.detail
    );
}

/// An unset id reads as `None`.
pub fn read_uuid_of(r: wire_uuid::Reader<'_>) -> Option<Uuid> {
    read_uuid(r)
}

/// An id that exists nowhere.
pub fn unknown_id() -> Uuid {
    Envelope::new("Document", "x").id.as_uuid()
}

/// The checksum the contract names for these bytes.
pub fn checksum(data: &[u8]) -> String {
    fs_files_core::format_checksum(&blake3::hash(data))
}

/// Start an upload; the capability and its id.
pub async fn start(
    c: &Caps,
    folder: Option<Uuid>,
    name: &str,
) -> capnp::Result<(fs_interfaces::files_capnp::upload::Client, String)> {
    let mut call = c.transfer.start_upload_request();
    write_uuid(call.get().init_folder(), folder);
    call.get().set_name(name);
    let answer = call.send().promise.await?;
    let r = answer.get()?;
    Ok((r.get_upload()?, r.get_id()?.to_string()?))
}

pub async fn received(upload: &fs_interfaces::files_capnp::upload::Client) -> u64 {
    let answer = upload.received_request().send().promise.await.unwrap();
    answer.get().unwrap().get_bytes()
}
