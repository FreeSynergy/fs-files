//! The parts of the local disk that differ between Linux and Windows, each a trait with two implementations (ADR 0043).
//! Pattern: Facade (declarations).

mod api;
mod facade;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub(crate) use api::{Durability, Moves, NameRules};
pub(crate) use facade::current;
