//! Where bytes are: the provider interface and its backends.
//!
//! Pattern: Strategy (declarations) — [`BlobBackend`] is the interface; [`LocalFolder`] is the
//! default provider. Shared pieces (key rules, checksum, locks) live next to it.

mod digest;
mod factory;
mod key;
mod local;
mod lock;
mod provider;
mod registry;
#[cfg(feature = "s3")]
mod s3;

pub use digest::{CHUNK, format_checksum};
pub use factory::{BackendConfig, BackendFactory, LocalFactory, check_backend};
pub use key::{MAX_KEY, MAX_SEGMENT, check_key};
pub use local::LocalFolder;
pub use provider::{BlobBackend, BlobInfo};
pub use registry::BackendRegistry;
#[cfg(feature = "s3")]
pub use s3::{
    MAX_PARTS, MIN_PART_SIZE, ObjectApi, ObjectEntry, ObjectStoreApi, S3Backend, S3Config,
    S3Factory, S3Options,
};
