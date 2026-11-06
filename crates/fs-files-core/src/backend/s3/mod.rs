//! The S3 backend: module map.
//!
//! Pattern: Strategy (declarations) — [`S3Backend`] is the provider; it talks to an [`ObjectApi`],
//! which `ObjectStoreApi` implements over the `object_store` crate.

mod adapter;
mod api;
mod backend;
mod config;
mod factory;
mod flush;
mod layout;
mod state;

pub use adapter::ObjectStoreApi;
pub use api::{ObjectApi, ObjectEntry};
pub use backend::{MAX_PARTS, MIN_PART_SIZE, S3Backend, S3Options};
pub use config::S3Config;
pub use factory::S3Factory;
