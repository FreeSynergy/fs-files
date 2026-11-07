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

/// What the command line and the start say. A sentence has named places (`{option}`), never pieces
/// joined: word order differs between languages (`coding.md` 2a).
#[allow(missing_docs)] // each sentence is its own documentation
pub mod cli {
    pub const NEEDS_VALUE: &str = "{option} needs a value";
    pub const UNKNOWN_OPTION: &str = "unknown option {option}";
    pub const BAD_VIEW: &str = "--view wants list or grid, not {value}";
    pub const NEEDS_SOCKET: &str =
        "--socket <path> is needed: the socket of a running fs-files-server";
    pub const KEEPS_NOTHING: &str = "{option} is not an option of the app: it keeps no files and no store. Start fs-files-server (it takes FS_FILES_ROOT for the folder and --data for its store) and give the app its --socket.";
    pub const DOWNLOAD_TOGETHER: &str = "--download <id> and --to <path> go together";
    pub const RELEASE_NEEDS_DOWNLOAD: &str = "--release-sensitive needs --download";
    pub const INTO_NEEDS_UPLOAD: &str = "--into needs --upload";
    pub const CANNOT_REACH: &str = "cannot reach the files server at {socket}: {why}";
    pub const LOOKED: &str = "Looked at the folder in {seconds} s: {added} taken over, {updated} changed, {missing} missing, {skipped} not taken over. Changes made outside appear after Rescan.";
    pub const LOOK_FAILED: &str = "Could not look at the folder: {why}";

    /// The help text.
    pub const HELP: &str = "fs-files-app: the files program

  --socket <path>    the socket of a running fs-files-server (needed). The app keeps no files and no
                     store of its own: the folder and the store belong to the server
  --no-rescan        do not ask the server to look at its folder when starting
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1300x800)
  --view <list|grid> the view to start with (default list)
  --upload <path>    upload this file and stop; repeat for more; a symbolic link is refused
  --into <id>        the folder to upload into (default: the top)
  --keep-both        on a taken name keep both: the upload gets a free name; nothing is replaced
  --download <id>    write this file and stop
  --to <path>        where to write it: a new file, or a folder to put it in; nothing is replaced
  --release-sensitive
                     this download may write a file marked sensitive

Changes made in the folder from outside only come in by a rescan: at the start, or the Rescan button.";
}

/// A sentence with its named places filled in: `{name}` becomes the value given for `name`. A place
/// without a value stays as it is.
pub fn fill(sentence: &str, values: &[(&str, &str)]) -> String {
    let mut out = sentence.to_string();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// The words of the workbench.
pub fn labels() -> FileLabels {
    let mut labels = FileLabels::default();
    labels.root = TITLE.to_string();
    labels
}
