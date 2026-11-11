//! The files server as the source of the file workbench: the type [`RemoteFileSource`] and what it is made of.
//!
//! Pattern: none — this file only declares modules and re-exports the public API.

mod adapter;
mod convert;
mod moves;
mod read;
mod write;

pub use adapter::RemoteFileSource;
