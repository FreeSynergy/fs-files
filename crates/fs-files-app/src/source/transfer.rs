//! Bytes in and out, through a path the person typed.
//!
//! Pattern: Adapter + Template method — all of it blocks the interface thread until it is done, measured
//! in `DESIGN.md`; there is no way yet to post from a worker. An upload goes in chunks; what arrived before
//! a failure stays as an unfinished upload. A download appears whole or not at all, never replaces a
//! file, and a marked file needs the person's release and is written readable by its owner only.

use std::io::{Read, Seek, SeekFrom};

use fs_components_app::{write_new_file, write_new_private_file};
use fs_components_workbenches::file_manager::{FileError, FileProblem, NameRule, PendingUpload};
use fs_files_core::{CHUNK, Upload};

use super::adapter::{FileAdapter, folder_of, id_of};
use super::change::rule_of;
use super::paths::{open_source, target_for};
use crate::words::say;

/// The biggest file written out for now: `write_new_file` takes its whole content at once.
const MOST_IN_MEMORY: u64 = 2 * 1024 * 1024 * 1024;

fn io_error(e: &std::io::Error) -> FileError {
    FileError::failed(e.to_string())
}

impl FileAdapter {
    pub(super) fn do_upload(
        &self,
        path: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (file, size, name) = open_source(path)?;
        let folder = folder_of(folder)?;
        let (upload, _) =
            self.wait(
                self.transfer
                    .start_upload_as(folder, &name, "", rule_of(rule)),
            )?;
        self.send(file, size, 0, upload.as_ref())
    }

    /// Write the rest of `file`, from `from`, and finish. A file that does not end where it said stays
    /// unfinished: nothing is made of it.
    fn send(
        &self,
        mut file: std::fs::File,
        size: u64,
        from: u64,
        upload: &dyn Upload,
    ) -> Result<String, FileError> {
        let mut buffer = vec![0; CHUNK];
        let mut at = from;
        loop {
            let got = file.read(&mut buffer).map_err(|e| io_error(&e))?;
            if got == 0 {
                break;
            }
            self.wait(upload.write(at, &buffer[..got]))?;
            at += got as u64;
        }
        if at != size {
            return Err(FileError::failed(say::GREW));
        }
        let file = self.wait(upload.finish())?;
        Ok(file.name)
    }

    pub(super) fn do_pending(&self) -> Result<Vec<PendingUpload>, FileError> {
        Ok(self
            .wait(self.transfer.pending_uploads())?
            .into_iter()
            .map(|u| PendingUpload::new(u.id, u.name, u.received))
            .collect())
    }

    /// The core cannot tell whether a file is the one an upload started from. All that can be checked
    /// here: its name and that it is not smaller (see "Data-loss paths" in `DESIGN.md`).
    pub(super) fn do_resume(&self, id: &str, path: &str) -> Result<String, FileError> {
        let (mut file, size, name) = open_source(path)?;
        let pending = self.wait(self.transfer.pending_uploads())?;
        let info = pending
            .iter()
            .find(|u| u.id == id)
            .ok_or_else(|| FileError::new(FileProblem::NotFound, say::GONE))?;
        if !same_name(&info.name, &name) {
            return Err(FileError::failed(say::NOT_THE_UPLOAD));
        }
        let upload = self.wait(self.transfer.resume_upload(id))?;
        let from = self.wait(upload.received())?;
        if size < from {
            return Err(FileError::failed(say::SMALLER));
        }
        file.seek(SeekFrom::Start(from)).map_err(|e| io_error(&e))?;
        self.send(file, size, from, upload.as_ref())
    }

    pub(super) fn do_drop(&self, id: &str) -> Result<(), FileError> {
        let upload = self.wait(self.transfer.resume_upload(id))?;
        self.wait(upload.abort())
    }

    pub(super) fn do_download(
        &self,
        key: &str,
        path: &str,
        release_marked: bool,
    ) -> Result<String, FileError> {
        let id = id_of(key)?;
        let file = self
            .wait(self.files.get(id))?
            .ok_or_else(|| FileError::new(FileProblem::NotFound, say::GONE))?;
        let marked = !file.sensitive.is_empty();
        if marked && !release_marked {
            return Err(FileError::new(FileProblem::MarkedNeedsRelease, say::MARKED));
        }
        if file.size > MOST_IN_MEMORY {
            return Err(FileError::failed(say::TOO_BIG));
        }
        let target = target_for(path, &file.name)?;
        let blob = self.wait(self.transfer.open(id))?;
        let bytes = self.read_all(blob.as_ref(), file.size)?;
        let write = if marked {
            write_new_private_file
        } else {
            write_new_file
        };
        write(&target, &bytes).map_err(FileError::failed)?;
        Ok(target.display().to_string())
    }

    fn read_all(&self, blob: &dyn fs_files_core::Blob, size: u64) -> Result<Vec<u8>, FileError> {
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        let chunk = u32::try_from(CHUNK).unwrap_or(u32::MAX);
        while (bytes.len() as u64) < size {
            let part = self.wait(blob.read(bytes.len() as u64, chunk))?;
            if part.is_empty() {
                return Err(FileError::new(FileProblem::ChangedOutside, say::SHORT_READ));
            }
            bytes.extend(part);
        }
        Ok(bytes)
    }
}

/// Is `file` the name the upload was started with? An upload that keeps both may have a number added.
fn same_name(upload: &str, file: &str) -> bool {
    if upload == file {
        return true;
    }
    let (stem, extension) = match upload.rfind('.') {
        Some(i) if i > 0 => upload.split_at(i),
        _ => (upload, ""),
    };
    stem.rfind(" (").is_some_and(|i| {
        let number = stem[i + 2..].strip_suffix(')').unwrap_or("x");
        number.bytes().all(|b| b.is_ascii_digit()) && format!("{}{extension}", &stem[..i]) == file
    })
}
