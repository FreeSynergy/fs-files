//! `fs-files-app`: the window of the files program (ADR 0005, 0009, 0063).
//!
//! A proxy to the server ([`FilesRemote`], which is a `FileSource` of the file manager workbench in
//! `fs-components`) and the window ([`gui::run`]). It imports no `blinc_*` crate.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod gui;
mod remote;
mod source;

pub use remote::FilesRemote;
