//! What the command line does without a window.
//!
//! Pattern: Facade — two small functions (upload, download) in front of the `FileSource` the window
//! uses too, so the same checks hold: no replace, no symbolic link followed, a marked file only with its
//! release. Why: the command line needs a sequence and a line to print, not the whole source.

use fs_components_workbenches::file_manager::{FileSource, NameRule};

use crate::words::{cli, fill};

/// Upload `paths` into `into`. Returns the lines to print.
///
/// # Errors
/// A message for the person, at the first that fails; those before it are done.
pub fn upload(
    source: &dyn FileSource,
    paths: &[String],
    into: Option<&str>,
    keep_both: bool,
) -> Result<Vec<String>, String> {
    let rule = if keep_both {
        NameRule::KeepBoth
    } else {
        NameRule::Refuse
    };
    let mut lines = Vec::new();
    for path in paths {
        let done = source.upload(path, into, rule).map_err(|e| {
            fill(
                cli::UPLOAD_FAILED,
                &[("path", path), ("why", &e.to_string())],
            )
        })?;
        lines.push(fill(cli::UPLOADED, &[("path", path), ("name", &done)]));
    }
    Ok(lines)
}

/// Write the file with this id to `to`. A marked file needs `release`.
///
/// # Errors
/// A message for the person; nothing was written.
pub fn download(
    source: &dyn FileSource,
    id: &str,
    to: &str,
    release: bool,
) -> Result<String, String> {
    source
        .download(id, to, release)
        .map(|done| fill(cli::WROTE, &[("name", &done)]))
        .map_err(|e| e.to_string())
}
