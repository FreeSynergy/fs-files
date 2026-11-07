//! `FileAdapter`: `dyn Files` and `dyn Transfer` for the workbench.
//!
//! Pattern: Adapter over the program's contract, driven with a tokio handle from the interface thread.
//! Over the socket: the providers are `RpcFiles` of `fs-files-client`, a call blocks the interface thread until
//! the server has answered (`DESIGN.md`). Errors keep their kind: the contract's named problems are read from `Error::problem`, never from the words.

use std::future::Future;
use std::sync::Arc;

use fs_components_workbenches::file_manager::{FileEntry, FileError, FileProblem, FolderInfo};
use fs_files_core::{Error, File, Files, Folder, Transfer};
use fs_interfaces::files_capnp::{
    CHANGED_OUTSIDE_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE,
};
use fs_store::ObjectId;
use tokio::runtime::Handle;

use crate::words::say;

/// The contract of the files program, for the workbench. Do not call it from inside the runtime: it
/// blocks the calling thread until the core has answered.
pub struct FileAdapter {
    pub(super) files: Arc<dyn Files>,
    pub(super) transfer: Arc<dyn Transfer>,
    pub(super) runtime: Handle,
}

impl FileAdapter {
    /// An adapter over providers of the contract, driven with `runtime`.
    pub fn new(files: Arc<dyn Files>, transfer: Arc<dyn Transfer>, runtime: Handle) -> Self {
        Self {
            files,
            transfer,
            runtime,
        }
    }

    pub(super) fn wait<T>(
        &self,
        future: impl Future<Output = fs_files_core::Result<T>>,
    ) -> Result<T, FileError> {
        self.runtime.block_on(future).map_err(|e| problem_of(&e))
    }
}

/// A key as an object id. A key the core could not have given names nothing.
pub(super) fn id_of(key: &str) -> Result<ObjectId, FileError> {
    key.parse()
        .map_err(|_| FileError::new(FileProblem::NotFound, say::GONE))
}

pub(super) fn folder_of(key: Option<&str>) -> Result<Option<ObjectId>, FileError> {
    key.map(id_of).transpose()
}

/// What the core said, in the workbench's terms: the kind comes from the contract's named problems.
pub fn problem_of(e: &Error) -> FileError {
    let problem = e.problem();
    let kind = match (&problem, e) {
        (Some(p), _) if p.kind == NAME_CONFLICT_PROBLEM_TYPE => FileProblem::NameConflict,
        (Some(p), _) if p.kind == INVALID_NAME_PROBLEM_TYPE => FileProblem::InvalidName,
        (Some(p), _) if p.kind == CHANGED_OUTSIDE_PROBLEM_TYPE => FileProblem::ChangedOutside,
        (_, Error::NotFound(_)) => FileProblem::NotFound,
        (_, Error::Unsupported(_)) => FileProblem::Unsupported,
        _ => FileProblem::Failed,
    };
    let detail = problem.map_or_else(|| e.to_string(), |p| p.detail);
    FileError::new(kind, readable(&detail))
}

/// The core's detail without the prefix that repeats the kind and the advice the workbench gives
/// itself. Only for reading: the kind above never comes from these words.
fn readable(detail: &str) -> String {
    let mut text = detail;
    for prefix in [
        "name taken: ",
        "invalid name: ",
        "changed outside: ",
        "backend: ",
    ] {
        text = text.strip_prefix(prefix).unwrap_or(text);
    }
    text.strip_suffix("; run rescan")
        .unwrap_or(text)
        .to_string()
}

pub(super) fn entry_of_file(file: &File) -> FileEntry {
    let mut entry = FileEntry::file(file.id.to_string(), file.name.clone(), file.size);
    entry.media_type.clone_from(&file.media_type);
    entry.sensitive.clone_from(&file.sensitive);
    entry.folder = file.folder.map(|f| f.to_string());
    entry.trashed = file.trashed;
    entry
}

pub(super) fn entry_of_folder(folder: &Folder) -> FileEntry {
    let mut entry = FileEntry::folder(folder.id.to_string(), folder.name.clone());
    entry.folder = folder.parent.map(|f| f.to_string());
    entry
}

pub(super) fn info_of(folder: &Folder) -> FolderInfo {
    FolderInfo::new(
        folder.id.to_string(),
        folder.name.clone(),
        folder.parent.map(|f| f.to_string()),
    )
}
