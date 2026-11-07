//! The command line.
//!
//! Pattern: none — a small pure function over the argument list (plain `std::env::args`, no extra crate
//! for a few options). The words it says are in `words.rs`.

use std::path::PathBuf;

use fs_components_app::parse_size;
use fs_components_workbenches::file_manager::FileView;

use crate::words::{cli, fill};

pub use crate::words::cli::HELP;

/// What was asked for on the command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// The socket of a running `fs-files-server`.
    pub socket: Option<PathBuf>,
    /// Draw a picture to this file instead of opening a window.
    pub snapshot: Option<PathBuf>,
    /// Size of the window or picture.
    pub size: (u32, u32),
    /// The view to start with.
    pub view: Option<FileView>,
    /// Do not look at the folder when starting.
    pub no_rescan: bool,
    /// Files to upload, then stop.
    pub upload: Vec<String>,
    /// The folder (its id) to upload into; the top if not given.
    pub into: Option<String>,
    /// On a taken name keep both; without it, a taken name stops with a message.
    pub keep_both: bool,
    /// The file (its id) to download, then stop.
    pub download: Option<String>,
    /// The path to download to, a new file.
    pub to: Option<String>,
    /// The person releases a file marked sensitive for this one download.
    pub release_sensitive: bool,
    /// `--help` was given.
    pub help: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            socket: None,
            snapshot: None,
            size: (1300, 800),
            view: None,
            no_rescan: false,
            upload: Vec::new(),
            into: None,
            keep_both: false,
            download: None,
            to: None,
            release_sensitive: false,
            help: false,
        }
    }
}

/// Parse `args` (without the program name).
///
/// # Errors
/// A message for the person: an unknown option, a missing value or a bad size or view.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| fill(cli::NEEDS_VALUE, &[("option", name)]))
        };
        match arg.as_str() {
            "--socket" => out.socket = Some(PathBuf::from(value("--socket")?)),
            "--root" | "--store" => return Err(fill(cli::KEEPS_NOTHING, &[("option", arg)])),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--size" => out.size = parse_size(&value("--size")?)?,
            "--view" => out.view = Some(view(&value("--view")?)?),
            "--no-rescan" => out.no_rescan = true,
            "--upload" => out.upload.push(value("--upload")?),
            "--into" => out.into = Some(value("--into")?),
            "--keep-both" => out.keep_both = true,
            "--download" => out.download = Some(value("--download")?),
            "--to" => out.to = Some(value("--to")?),
            "--release-sensitive" => out.release_sensitive = true,
            "-h" | "--help" => out.help = true,
            other => return Err(fill(cli::UNKNOWN_OPTION, &[("option", other)])),
        }
    }
    check(&out)?;
    Ok(out)
}

fn check(out: &Args) -> Result<(), String> {
    if out.help {
        return Ok(());
    }
    if out.socket.is_none() {
        return Err(cli::NEEDS_SOCKET.into());
    }
    if out.download.is_some() != out.to.is_some() {
        return Err(cli::DOWNLOAD_TOGETHER.into());
    }
    if out.release_sensitive && out.download.is_none() {
        return Err(cli::RELEASE_NEEDS_DOWNLOAD.into());
    }
    if out.into.is_some() && out.upload.is_empty() {
        return Err(cli::INTO_NEEDS_UPLOAD.into());
    }
    Ok(())
}

fn view(text: &str) -> Result<FileView, String> {
    match text {
        "list" => Ok(FileView::List),
        "grid" => Ok(FileView::Grid),
        other => Err(fill(cli::BAD_VIEW, &[("value", other)])),
    }
}
