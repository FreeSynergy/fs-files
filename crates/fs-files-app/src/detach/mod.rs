//! Starting a background process that outlives the window, one implementation per system. Pattern: Facade (declarations).

mod api;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use api::{Detach, current};
