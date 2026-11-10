//! Pattern: Factory + Facade (the parts of this system, built once).

use std::sync::OnceLock;

use super::{Durability, Moves, NameRules};

/// The parts of the disk of this system.
pub(crate) struct Platform {
    /// Which names it takes.
    pub names: Box<dyn NameRules>,
    /// Durability of folder changes.
    pub durability: Box<dyn Durability>,
    /// Moving without replacing.
    pub moves: Box<dyn Moves>,
}

/// The platform of the machine this runs on.
pub(crate) fn current() -> &'static Platform {
    static PLATFORM: OnceLock<Platform> = OnceLock::new();
    PLATFORM.get_or_init(build)
}

#[cfg(unix)]
fn build() -> Platform {
    super::unix::platform()
}

#[cfg(windows)]
fn build() -> Platform {
    super::windows::platform()
}
