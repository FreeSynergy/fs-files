//! `fs-files-client`: the files program as a client of its own contract `files.capnp`.
//!
//! [`RpcFiles`] implements [`fs_files_core::Files`] and [`fs_files_core::Transfer`] over a Unix socket,
//! so a frontend depends on the traits and talks to a running `fs-files-server`, never to the core of
//! the program (ADR 0021). What a method does not offer answers the `Unsupported` of the provider rule
//! (ADR 0045); `DESIGN.md` lists which.
//!
//! Interface status: draft (ADR 0045), as the contract.
//!
//! Pattern: Facade — this file only declares modules and re-exports the public API.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]
// A closure reads better than the full path of a generated reader type.
#![allow(
    clippy::module_name_repetitions,
    clippy::redundant_closure_for_method_calls
)]

mod cursor;
mod file_calls;
mod files_impl;
mod folder_calls;
mod peer;
mod problems;
#[cfg(test)]
mod problems_table;
mod read;
mod remote;
mod transfer_calls;
mod transfer_impl;
mod write;

pub use fs_interfaces::rpc::client::{CALL_TIMEOUT, CONNECT_TIMEOUT, Timeouts};
pub use peer::RpcFiles;
