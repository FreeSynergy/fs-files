//! What the command line does without a window.
//!
//! Pattern: Facade consumer — the same adapter the window uses, so the same checks hold: no replace, no
//! symbolic link followed, a marked file only with its release.

use fs_components_workbenches::file_manager::{FileSource, NameRule};

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
        let done = source
            .upload(path, into, rule)
            .map_err(|e| format!("{path}: {e}"))?;
        lines.push(format!("uploaded {path} as {done}"));
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
        .map(|done| format!("wrote {done}"))
        .map_err(|e| e.to_string())
}
