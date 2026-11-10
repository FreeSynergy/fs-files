//! What the path part of a `file:` link means on this system, one implementation per system. Pattern: Facade (declarations).

mod api;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use api::{FileLinks, current};
