//! `fs-files-app`: the window of the files program (ADR 0005, 0009, 0063, 0065, 0066).
//!
//! A proxy to the server ([`FilesRemote`]), the `Backend` of each data type for the generated Admin frontend
//! (`fs-admin-frontend`) and the window ([`gui::run`]). It imports no `blinc_*` crate and draws nothing itself.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    // the generated readers have one method per field; a closure names the one asked for
    clippy::redundant_closure_for_method_calls,
    clippy::needless_pass_by_value
)]

mod backend_file;
mod backend_folder;
mod backend_picture;
mod backend_storage;
mod catalog;
mod dial;
mod faces;
pub mod gui;
mod limits;
mod link;
mod listing;
mod remote;
pub mod settings;
mod source;
mod transfer;
mod uploads;
mod values;
pub mod window;
mod words;
mod xdg;

pub use backend_file::FileBackend;
pub use backend_folder::FolderBackend;
pub use backend_picture::{AlbumBackend, PictureBackend};
pub use backend_storage::StorageBackendBackend;
pub use limits::Limits;
pub use remote::FilesRemote;
pub use source::RemoteFileSource;
