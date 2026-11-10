//! Pattern: Strategy (Unix: the text is the path).

use std::path::PathBuf;

use super::FileLinks;

pub(super) struct PlainPaths;

impl FileLinks for PlainPaths {
    fn path_of(&self, text: &str) -> Option<PathBuf> {
        Some(PathBuf::from(text))
    }
}
