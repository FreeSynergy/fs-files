//! `fs-files-app`: the window of the files program (ADR 0005, 0009, 0063, 0065, 0066).
//!
//! A proxy to the server ([`FilesRemote`]), the `Backend` of each data type for the generated Admin frontend
//! (`fs-admin-frontend`) and the window ([`gui::run`]). It imports no `blinc_*` crate and draws nothing itself.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

mod backend_file;
mod backend_folder;
mod backend_picture;
mod backend_storage;
mod catalog;
pub mod gui;
mod listing;
mod remote;
mod transfer;
mod values;
pub mod window;
mod words;

pub use backend_file::FileBackend;
pub use backend_folder::FolderBackend;
pub use backend_picture::{AlbumBackend, PictureBackend};
pub use backend_storage::StorageBackendBackend;
pub use remote::FilesRemote;
