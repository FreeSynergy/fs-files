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
mod feed;
mod files;
mod model;
mod repo;
mod transfer;

pub use backend::{
    BackendConfig, BackendFactory, BackendRegistry, BlobBackend, BlobInfo, CHUNK,
    ConfinedLocalFactory, LocalFactory, LocalFolder, MAX_KEY, MAX_SEGMENT, Naming, Place,
    RESERVED_NAME, ScanKind, Scanned, Spot, check_backend, check_key, format_checksum,
};
#[cfg(feature = "s3")]
pub use backend::{
    MAX_PARTS, MIN_PART_SIZE, ObjectApi, ObjectEntry, ObjectStoreApi, S3Backend, S3Config,
    S3Factory, S3Options,
};
pub use backends::Backends;
pub use error::{Error, Problem, Result};
pub use feed::{FileChange, FileFeed, FileListener, Registry, Subscription};
pub use files::Files;
pub use model::{
    BackendInfo, Cursor, Entry, File, FileFilter, Folder, OnConflict, Page, RescanReport, Sort,
    SortBy, TrashReport, UploadInfo,
};
pub use repo::{CONTEXT, FOLDER_TYPE, FileRepo, MAX_NAME, UPLOAD_TYPE, check_name};
pub use transfer::{Blob, Transfer, Upload};
