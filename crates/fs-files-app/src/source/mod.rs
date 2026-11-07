//! `FileAdapter`: the program's contract as the workbench's `FileSource`.

mod adapter;
mod change;
mod listing;
mod paths;
mod provider;
mod transfer;

pub use adapter::{FileAdapter, problem_of};
