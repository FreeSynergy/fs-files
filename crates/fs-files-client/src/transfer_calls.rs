//! The calls of `Transfer`, `Upload` and `Blob`, one function each, on the connection's thread.
//!
//! Pattern: Adapter — one request in, one reply read back. An upload or a blob is named again by its
//! id in every call (`resumeUpload`, `open`): a capability cannot leave the thread of the connection,
//! and an upload is made to be taken up again by its id, so nothing is held between two calls.

use fs_files_core::{File, OnConflict, Result, UploadInfo};
use fs_interfaces::common_capnp::blob;
use fs_interfaces::files_capnp::{transfer, upload};
use fs_interfaces::rpc::write_object_id;
use fs_store::ObjectId;

use crate::problems::error_of;
use crate::read::{self, unreadable};
use crate::write;

/// `startUpload`: the id of the new session; the capability is let go, the session is kept by the
/// server and taken up again by id.
pub(crate) async fn start(
    c: &transfer::Client,
    folder: Option<ObjectId>,
    name: String,
    media_type: String,
    on: OnConflict,
) -> Result<String> {
    let mut req = c.start_upload_request();
    write::top(req.get().init_folder(), folder);
    req.get().set_name(name.as_str());
    req.get().set_media_type(media_type.as_str());
    req.get().set_on_conflict(write::conflict(on));
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    let id = answer.get().and_then(|r| r.get_id()).map_err(unreadable)?;
    id.to_string().map_err(unreadable)
}

/// `resumeUpload`: the capability of the upload with this id.
pub(crate) async fn resume(c: &transfer::Client, id: String) -> Result<upload::Client> {
    let mut req = c.resume_upload_request();
    req.get().set_id(id.as_str());
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    answer
        .get()
        .and_then(|r| r.get_upload())
        .map_err(unreadable)
}

/// `Upload.write`.
pub(crate) async fn write_at(u: &upload::Client, offset: u64, data: Vec<u8>) -> Result<()> {
    let mut req = u.write_request();
    req.get().set_offset(offset);
    req.get().set_data(&data);
    req.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(())
}

/// `Upload.received`.
pub(crate) async fn received(u: &upload::Client) -> Result<u64> {
    let answer = u
        .received_request()
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e))?;
    Ok(answer.get().map_err(unreadable)?.get_bytes())
}

/// `Upload.finish`.
pub(crate) async fn finish(u: &upload::Client) -> Result<File> {
    let answer = u
        .finish_request()
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e))?;
    read::file(
        answer
            .get()
            .and_then(|r| r.get_file())
            .map_err(unreadable)?,
    )
}

/// `Upload.abort`.
pub(crate) async fn abort(u: &upload::Client) -> Result<()> {
    u.abort_request()
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e))?;
    Ok(())
}

/// `pendingUploads`, every page until the cursor is empty. A page that is empty but names a next one
/// cannot be followed to an end, so it is an error instead of a silently short list.
pub(crate) async fn pending(c: &transfer::Client) -> Result<Vec<UploadInfo>> {
    let (mut found, mut cursor) = (Vec::new(), Vec::new());
    loop {
        let mut req = c.pending_uploads_request();
        req.get().init_cursor().set_opaque(&cursor);
        let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
        let r = answer.get().map_err(unreadable)?;
        let before = found.len();
        for item in r.get_items().map_err(unreadable)? {
            found.push(read::upload(item)?);
        }
        cursor = r
            .get_next()
            .and_then(|n| n.get_opaque())
            .map_err(unreadable)?
            .to_vec();
        if cursor.is_empty() {
            return Ok(found);
        }
        if found.len() == before {
            return Err(unreadable("an empty page that names a next one"));
        }
    }
}

/// `open`: the capability to read the bytes of a file.
pub(crate) async fn open(c: &transfer::Client, id: ObjectId) -> Result<blob::Client> {
    let mut req = c.open_request();
    write_object_id(req.get().init_id(), id);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    answer.get().and_then(|r| r.get_blob()).map_err(unreadable)
}

/// `Blob.size`.
pub(crate) async fn size(b: &blob::Client) -> Result<(u64, String)> {
    let answer = b
        .size_request()
        .send()
        .promise
        .await
        .map_err(|e| error_of(&e))?;
    let r = answer.get().map_err(unreadable)?;
    let media = r.get_media_type().map_err(unreadable)?;
    Ok((r.get_bytes(), media.to_string().map_err(unreadable)?))
}

/// `Blob.read`.
pub(crate) async fn read_range(b: &blob::Client, offset: u64, length: u32) -> Result<Vec<u8>> {
    let mut req = b.read_request();
    req.get().set_offset(offset);
    req.get().set_length(length);
    let answer = req.send().promise.await.map_err(|e| error_of(&e))?;
    Ok(answer
        .get()
        .and_then(|r| r.get_data())
        .map_err(unreadable)?
        .to_vec())
}
