//! The words of the program.
//!
//! Pattern: Value object — English only for now (ADR 0053). The workbench's own words are its defaults;
//! what the program itself says is here.

use fs_components_workbenches::file_manager::FileLabels;

/// The title of the window.
pub(crate) const TITLE: &str = "Files";

/// The words of the menus.
pub(crate) mod menu {
    pub const FILE: &str = "File";
    pub const NEW_FOLDER: &str = "New folder";
    pub const UPLOAD: &str = "Upload";
    pub const RESCAN: &str = "Rescan";
    pub const VIEW: &str = "View";
    pub const LIST: &str = "List";
    pub const GRID: &str = "Grid";
    pub const TRASH: &str = "Trash";
    pub const TRANSFERS: &str = "Transfers";
}

/// What the adapter says when a path cannot be used.
pub(crate) mod say {
    pub const NO_PATH: &str = "No path was given.";
    pub const NO_FILE_NAME: &str = "This path does not name a file.";
    pub const NOT_UTF8: &str = "The name of this file is not valid text, so it cannot be kept.";
    pub const SYMLINK: &str = "This path is a symbolic link; it is not followed.";
    pub const NOT_A_FILE: &str = "This path is not a plain file.";
    pub const SWAPPED: &str = "The file at this path changed while it was opened.";
    pub const GREW: &str =
        "The file changed while it was read; what arrived is kept as an unfinished upload.";
    pub const TAKEN_PATH: &str = "Something is already at this path. Nothing was replaced.";
    pub const NO_PARENT: &str = "The folder of this path does not exist.";
    pub const TOO_BIG: &str =
        "This file is too big to write here for now: it would have to fit in memory.";
    pub const SHORT_READ: &str = "The file ended before its recorded size.";
    pub const MARKED: &str =
        "This file is marked sensitive: writing it out needs an explicit release.";
    pub const NOT_THE_UPLOAD: &str =
        "This file has another name than the upload; it was not added to it.";
    pub const SMALLER: &str =
        "This file is smaller than what already arrived; it cannot be the same file.";
    pub const GONE: &str = "This is no longer there.";
    pub const BAD_DAY: &str = "A date could not be read.";
}

/// The words of the workbench.
pub fn labels() -> FileLabels {
    let mut labels = FileLabels::default();
    labels.root = TITLE.to_string();
    labels
}
