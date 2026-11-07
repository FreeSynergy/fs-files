//! The contract's types on the wire.
//!
//! Pattern: Facade (declarations) — one concept per file: ids and enums, texts, cursors, entries,
//! filters, reports.

pub(crate) mod cursor;
pub(crate) mod entry;
pub(crate) mod filter;
pub(crate) mod ids;
pub(crate) mod report;
pub(crate) mod text;

/// The version `supportReport` names.
pub(crate) const CONTRACT_VERSION: &str = "files 0.2 (draft)";

/// A length as the `u32` capnp lists take.
pub(crate) fn count(n: usize) -> capnp::Result<u32> {
    u32::try_from(n).map_err(|_| capnp::Error::failed("list too long".into()))
}
