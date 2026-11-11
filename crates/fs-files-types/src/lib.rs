//! `fs-files-types`: what the files program tells the world about its data types (ADR 0066).
//!
//! Pattern: none — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

mod album;
mod common;
mod facet;
mod file;
mod file_later;
mod folder;
mod picture;
mod picture_later;
mod registry;
mod requirements;
mod storage;
mod support;
mod wire;

pub use album::Album;
pub use facet::{FACETS, check_facet, facet_types};
pub use file::File;
pub use folder::Folder;
pub use picture::Picture;
pub use registry::registry;
pub use requirements::{REQUIREMENTS, Requirement};
pub use storage::StorageBackend;
pub use support::{SUPPORTED, is_supported, unsupported};
pub use wire::{read_catalog, write_catalog};
