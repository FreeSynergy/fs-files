//! `fs-files-core`: program core of FreeSynergy files (no GUI).
//!
//! What we know about a file is an object in `fs-store` (AS2 `Document`, folders are `Collection`);
//! its bytes are in a backend behind [`BlobBackend`]. Other programs refer to a file by id and read
//! it through [`Blob`]; they never see paths or backends. Start with the traits [`Files`],
//! [`Transfer`] and [`Backends`]; [`FileRepo`] implements them over a store. See `DESIGN.md`.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// Pedantic lints that would only add noise to this crate's small, documented API:
// every fallible fn returns the crate `Result` (documented once on `Error`).
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions
)]

mod backend;
mod backends;
mod error;
mod files;
mod model;
mod repo;
mod transfer;

pub use backend::{
    BackendConfig, BackendFactory, BackendRegistry, BlobBackend, BlobInfo, CHUNK, LocalFactory,
    LocalFolder, MAX_KEY, MAX_SEGMENT, check_backend, check_key, format_checksum,
};
pub use backends::Backends;
pub use error::{Error, Result};
pub use files::Files;
pub use model::{BackendInfo, Cursor, Entry, File, FileFilter, Folder, Page};
pub use repo::{CONTEXT, FOLDER_TYPE, FileRepo, MAX_NAME, UPLOAD_TYPE, check_name};
pub use transfer::{Blob, Transfer, Upload};
