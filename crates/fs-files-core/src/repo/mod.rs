//! The store-backed files program: module map.
//!
//! Pattern: Facade (declarations) — `FileRepo` is the one entry point; the files behind it are
//! one concept each (folders, file operations, upload sessions, listing, backends).

mod backend;
mod blob;
mod build;
mod consts;
mod fileops;
mod finish;
mod folders;
mod handle;
mod list;
mod lookup;
mod manage;
mod names;
mod provider;
mod session;

pub use consts::{CONTEXT, FOLDER_TYPE, UPLOAD_TYPE};
pub use handle::FileRepo;
pub use names::{MAX_NAME, check_name};
