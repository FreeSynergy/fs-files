//! The local folder backend: module map.
//!
//! Pattern: Strategy (declarations) — [`LocalFolder`] is the provider; path rules and durable IO
//! are in their own files.

mod folder;
mod io;
mod named;
mod path;

pub use folder::LocalFolder;
