//! The command line.
//!
//! Pattern: Builder-like parser — plain `std::env::args`, no extra crate for a few options.

use std::path::PathBuf;

use fs_components_app::parse_size;
use fs_components_workbenches::file_manager::FileView;

/// What was asked for on the command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    /// Store file; `None` = in memory, gone when the program ends.
    pub store: Option<PathBuf>,
    /// The folder the local backend keeps the files in.
    pub root: Option<PathBuf>,
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
            store: None,
            root: None,
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
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--store" => out.store = Some(PathBuf::from(value("--store")?)),
            "--root" => out.root = Some(PathBuf::from(value("--root")?)),
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
            other => return Err(format!("unknown option {other}")),
        }
    }
    check(&out)?;
    Ok(out)
}

fn check(out: &Args) -> Result<(), String> {
    if out.help {
        return Ok(());
    }
    if out.root.is_none() {
        return Err("--root <folder> is needed: the folder that holds the files".into());
    }
    if out.download.is_some() != out.to.is_some() {
        return Err("--download <id> and --to <path> go together".into());
    }
    if out.release_sensitive && out.download.is_none() {
        return Err("--release-sensitive needs --download".into());
    }
    if out.into.is_some() && out.upload.is_empty() {
        return Err("--into needs --upload".into());
    }
    Ok(())
}

fn view(text: &str) -> Result<FileView, String> {
    match text {
        "list" => Ok(FileView::List),
        "grid" => Ok(FileView::Grid),
        other => Err(format!("--view wants list or grid, not {other}")),
    }
}

/// The help text.
pub const HELP: &str = "fs-files-app: the files program

  --root <folder>    the folder that holds the files (needed); files there keep their real names
  --store <file>     keep what is known about the files in this store file (default: in memory, gone
                     at the end)
  --no-rescan        do not look at the folder when starting: only what the store knows shows
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
