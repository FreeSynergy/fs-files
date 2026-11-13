//! What the window keeps of the choices of the person: today the face of the file window (list or grid).
//!
//! Pattern: Memento — one small text file `files-window.conf` (`view=list`) in the data folder of the person; written
//! whole to a file next to it and renamed over the old one (atomic: a crash leaves the old or the new file, never half
//! of one); a file that is missing or unreadable is the default, never an error.

use std::path::{Path, PathBuf};

use fs_components_workbenches::file_manager::FileView;

/// The file of the settings, inside the data folder.
pub const FILE: &str = "files-window.conf";

/// The folder the settings live in: `$XDG_DATA_HOME/freesynergy`, else `~/.local/share/freesynergy`, else (Windows) the
/// local application data folder. `None` where the system names none.
pub fn data_folder() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        })?;
    Some(base.join("freesynergy"))
}

/// The usual place of the settings file.
pub fn default_path() -> Option<PathBuf> {
    data_folder().map(|d| d.join(FILE))
}

/// The word of a view in the file.
fn word(view: FileView) -> &'static str {
    match view {
        FileView::Grid => "grid",
        _ => "list",
    }
}

/// The view the text names; anything else (empty, damaged, an unknown word) is `None`.
fn parse(text: &str) -> Option<FileView> {
    text.lines()
        .find_map(|line| match line.trim().strip_prefix("view=")?.trim() {
            "list" => Some(FileView::List),
            "grid" => Some(FileView::Grid),
            _ => None,
        })
}

/// The kept view; none kept, or a file that cannot be read or understood, gives the standard (the list).
pub fn load(path: &Path) -> FileView {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| parse(&t))
        .unwrap_or(FileView::List)
}

/// Keep `view`: written to a file next to `path` and renamed over it.
///
/// # Errors
/// The message for the person when the folder or the file cannot be written.
pub fn save(path: &Path, view: FileView) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("cannot keep the view in {}: {e}", path.display());
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(fail)?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(format!(".{}.part", std::process::id()));
    let part = PathBuf::from(part);
    std::fs::write(&part, format!("view={}\n", word(view))).map_err(fail)?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        fail(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_view_comes_back_and_a_second_save_replaces_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join(FILE);
        save(&path, FileView::Grid).unwrap();
        assert_eq!(load(&path), FileView::Grid);
        save(&path, FileView::List).unwrap();
        assert_eq!(load(&path), FileView::List);
        let left: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, [std::ffi::OsString::from(FILE)], "no part file stays");
    }

    #[test]
    fn a_missing_or_damaged_file_is_the_standard() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        assert_eq!(load(&path), FileView::List, "missing");
        for text in ["", "view=", "view=tiles", "\u{0}\u{1}garbage", "grid"] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(load(&path), FileView::List, "damaged: {text:?}");
        }
        std::fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(load(&path), FileView::List, "not text");
        std::fs::write(&path, "# mine\nview=grid\n").unwrap();
        assert_eq!(load(&path), FileView::Grid, "a comment line does no harm");
    }

    #[test]
    fn a_folder_that_cannot_be_made_is_told() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        let err = save(&blocker.join("sub").join(FILE), FileView::Grid).unwrap_err();
        assert!(err.contains("cannot keep the view"), "{err}");
    }
}
