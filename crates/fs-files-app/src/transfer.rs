//! Upload and download between a local path (a `file:` link typed in the form) and a file of the program.
//!
//! Pattern: Template method — a fixed flow (open, move the bytes in chunks, close), the same for both directions; the
//! bytes never pass through memory as a whole. The form has no file chooser yet, so a path is given as a `file:` link
//! (`BasicType::Link`); a chooser is a component in `fs-components` and belongs to the frontend (named in `DESIGN.md`).

use std::io::{Read, Write};
use std::path::PathBuf;

use fs_admin_frontend::{BackendError, Outcome, Values};
use fs_interfaces::files_capnp::transfer;
use fs_interfaces::rpc::write_uuid;
use fs_types::{RecordIssue, RecordProblem, TypeProblem};
use uuid::Uuid;

use crate::remote::problem;
use crate::values::{id_of, on_conflict, text};

/// Bytes per call: well below the server's 16 MiB intake and 32 MiB answer.
const CHUNK: usize = 1 << 20;

fn rejected(field: &str, problem: TypeProblem) -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: field.to_owned(),
        index: 0,
        problem: RecordProblem::Value(problem),
    }])
}

fn hex(b: u8) -> Option<u8> {
    char::from(b)
        .to_digit(16)
        .and_then(|d| u8::try_from(d).ok())
}

/// The path a `file:` link names: `file:///home/me/a.jpg`, host empty or `localhost`, percent codes decoded.
pub(crate) fn path_of(input: &Values, field: &str) -> Result<PathBuf, BackendError> {
    let link = text(input, field).ok_or_else(|| rejected(field, TypeProblem::Empty))?;
    let rest = link
        .strip_prefix("file://")
        .ok_or_else(|| rejected(field, TypeProblem::NotAbsolute))?;
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') {
        return Err(rejected(field, TypeProblem::BadUri));
    }
    let raw = path.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let decoded = (raw[i] == b'%')
            .then(|| Some((hex(*raw.get(i + 1)?)? << 4) | hex(*raw.get(i + 2)?)?))
            .flatten();
        if let Some(byte) = decoded {
            out.push(byte);
            i += 3;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(out)
        .map(PathBuf::from)
        .map_err(|_| rejected(field, TypeProblem::BadUri))
}

fn io(e: &std::io::Error) -> BackendError {
    BackendError::Unavailable(format!("local file: {e}"))
}

/// Upload the file at `source` into `folder` (none = top level) and answer the new record.
pub(crate) async fn upload(t: &transfer::Client, input: &Values) -> Result<Outcome, BackendError> {
    let path = path_of(input, "source")?;
    let name = text(input, "name")
        .map(str::to_owned)
        .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .ok_or_else(|| rejected("name", TypeProblem::Empty))?;
    let mut local = std::fs::File::open(&path).map_err(|e| io(&e))?;
    let mut start = t.start_upload_request();
    {
        let mut p = start.get();
        write_uuid(p.reborrow().init_folder(), id_of(input, "folder")?);
        p.set_name(name.as_str());
        p.set_media_type(text(input, "mediaType").unwrap_or_default());
        p.set_on_conflict(on_conflict(input));
    }
    let answer = start.send().promise.await.map_err(|e| problem(&e))?;
    let upload = answer
        .get()
        .and_then(|r| r.get_upload())
        .map_err(|e| problem(&e))?;
    let mut offset = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let n = local.read(&mut buffer).map_err(|e| io(&e))?;
        if n == 0 {
            break;
        }
        let mut write = upload.write_request();
        write.get().set_offset(offset);
        write.get().set_data(&buffer[..n]);
        if let Err(e) = write.send().promise.await {
            let _ = upload.abort_request().send().promise.await;
            return Err(problem(&e));
        }
        offset += n as u64;
    }
    let answer = upload
        .finish_request()
        .send()
        .promise
        .await
        .map_err(|e| problem(&e))?;
    let file = answer
        .get()
        .and_then(|r| r.get_file())
        .map_err(|e| problem(&e))?;
    let record = crate::values::file_record(file).map_err(|e| problem(&e))?;
    Ok(Outcome {
        id: Some(record.id.clone()),
        values: Values::from([("created".to_owned(), vec![record.id])]),
    })
}

/// Download the bytes of file `id` into the new file at `target`; an existing file is never replaced.
pub(crate) async fn download(
    t: &transfer::Client,
    id: Uuid,
    input: &Values,
) -> Result<Outcome, BackendError> {
    let path = path_of(input, "target")?;
    let mut open = t.open_request();
    write_uuid(open.get().init_id(), Some(id));
    let answer = open.send().promise.await.map_err(|e| problem(&e))?;
    let blob = answer
        .get()
        .and_then(|r| r.get_blob())
        .map_err(|e| problem(&e))?;
    let mut local = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| io(&e))?;
    let mut offset = 0u64;
    loop {
        let mut read = blob.read_request();
        read.get().set_offset(offset);
        read.get()
            .set_length(u32::try_from(CHUNK).unwrap_or(u32::MAX));
        let answer = match read.send().promise.await {
            Ok(a) => a,
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                return Err(problem(&e));
            }
        };
        let data = answer
            .get()
            .and_then(|r| r.get_data())
            .map_err(|e| problem(&e))?;
        if data.is_empty() {
            break;
        }
        local.write_all(data).map_err(|e| io(&e))?;
        offset += data.len() as u64;
    }
    local.sync_all().map_err(|e| io(&e))?;
    Ok(Outcome {
        id: Some(id.to_string()),
        values: Values::new(),
    })
}
