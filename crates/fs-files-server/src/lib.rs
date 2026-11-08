//! `fs-files-server`: the contract `files.capnp` over a Unix socket, in front of any provider of
//! `Files`, `Transfer`, `Backends` and `FileFeed`. It only translates: no file logic lives here.
//!
//! Interface status: draft, provisional (ADR 0045).
//!
//! Pattern: Facade — this file only declares modules and re-exports.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// The generated `Server` traits are async for every method, also those that never wait.
#![allow(clippy::missing_errors_doc, clippy::unused_async_trait_impl)]

mod backend_service;
mod backend_wire;
mod backends_service;
mod blob_service;
mod errors;
mod facts;
mod feed;
mod file_ops;
mod files_service;
mod folder_ops;
mod folders_service;
pub mod lease;
mod places;
mod program;
mod search_service;
mod transfer_service;
mod trash_ops;
mod upload_service;
mod wire;

pub use backend_service::BlobBackendServer;
pub use files_service::FilesServer;
pub use places::check_places;
pub use program::{HELP, ServeError, serve};
