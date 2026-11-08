//! `FileSource` for [`FilesRemote`]: the file manager workbench over `files.capnp`, split by responsibility.
//!
//! Pattern: Adapter — `contract.rs` is the `FileSource` impl, `browse.rs`/`change.rs`/`housekeeping.rs`
//! hold the contract calls, `convert.rs` the conversions.
//! What this window does not do yet answers `Unsupported` and is not offered (capabilities): previews,
//! upload, download. A sensitive marking of a file (ADR 0029) is not read yet, so entries show none.

mod browse;
mod change;
mod contract;
mod convert;
mod housekeeping;
