//! The unfinished uploads of the program: list them and abort one.
//!
//! Pattern: Adapter — `Transfer.pendingUploads` and `Upload.abort` of `files.capnp` as commands of the type `File`.

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_interfaces::files_capnp::transfer;

use crate::listing::MAX_RECORDS;
use crate::remote::problem;
use crate::values::text;

/// The unfinished uploads, page after page (oldest first), at most [`MAX_RECORDS`].
pub(crate) async fn pending(t: &transfer::Client) -> Result<Outcome, BackendError> {
    let mut out = Outcome::default();
    let mut cursor: Vec<u8> = Vec::new();
    let mut seen = 0usize;
    loop {
        let mut call = t.pending_uploads_request();
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.map_err(|e| problem(&e))?;
        let r = answer.get().map_err(|e| problem(&e))?;
        for u in r.get_items().map_err(|e| problem(&e))? {
            let text = |t: capnp::Result<capnp::text::Reader<'_>>| {
                t.map_err(|e| problem(&e))
                    .and_then(|t| t.to_string().map_err(|e| problem(&e.into())))
            };
            out.values
                .entry("uploadId".into())
                .or_default()
                .push(text(u.get_id())?);
            out.values
                .entry("name".into())
                .or_default()
                .push(text(u.get_name())?);
            out.values
                .entry("received".into())
                .or_default()
                .push(u.get_received().to_string());
            seen += 1;
        }
        let next = r
            .get_next()
            .and_then(fs_interfaces::common_capnp::cursor::Reader::get_opaque)
            .map_err(|e| problem(&e))?;
        if next.is_empty() || seen >= MAX_RECORDS {
            return Ok(out);
        }
        cursor = next.to_vec();
    }
}

pub(crate) async fn abort(t: &transfer::Client, input: &Values) -> Result<Outcome, BackendError> {
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
