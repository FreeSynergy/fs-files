//! `fs-files-app`: the frontend of the files program (ADR 0005, 0009, 0037).
//!
//! Arguments, the words, and an adapter from the program's contract to the workbench's `FileSource`.
//! It imports no `blinc_*` crate: the window and the work surface come from `fs-components`. It keeps
//! no files and no store: it talks to a running `fs-files-server` over its socket (`fs-files-client`).
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate, clippy::module_name_repetitions)]

pub mod args;
pub mod commands;
mod source;
mod surface;
mod words;

pub use source::{FileAdapter, problem_of};
pub use surface::{FilesScreen, app_widget, screen_widget};
pub use words::{cli, fill, labels, tenths};
