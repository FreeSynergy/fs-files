//! The start of `fs-files` (ADR 0063): wiring only.
//!
//! Pattern: Facade consumer — connect to the server (the version handshake: a server of another version is
//! replaced, none is started; a failure is shown in the window), build the file manager, open the window (`window.rs`) or
//! draw one picture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_components_app::{WindowOptions, launch, parse_size};
use fs_interfaces::rpc::{CommandStarter, companion_path, default_socket, parse_facet};

use crate::FilesRemote;
use crate::settings;
use crate::window::{app_widget, files_widget};
use fs_components_workbenches::file_manager::FileView;

/// The help text.
pub const HELP: &str = "fs-files: the files window (without arguments it opens)

  --socket <path>    the files server to talk to (default: $XDG_RUNTIME_DIR/freesynergy/files.sock);
                     a server that is not there, or of another version, is replaced by one of this version
  --snapshot <png>   draw a picture instead of opening a window
  --view <list|grid> how the file window starts (default: the face chosen last in the View menu, else list)
  --rescan           first take over what lies on the disk of the files folder (what Rescan does)
  --admin            the generated Admin frontend instead of the file window (the same as --facet admin)
  --facet <name>     admin (all types), or one facet of the Admin frontend: files (files, folders, storage backends)
                     or gallery (pictures, albums); without --facet and --admin the file window opens
  --size <WxH>       window or picture size (default 1200x800)";

/// The descriptions the window shows: all, or the types of one facet.
fn in_facet(
    descriptions: Vec<fs_types::TypeDescription>,
    facet: Option<&str>,
) -> Vec<fs_types::TypeDescription> {
    let Some(types) = facet.and_then(fs_files_types::facet_types) else {
        return descriptions;
    };
    descriptions
        .into_iter()
        .filter(|d| types.contains(&d.name.as_str()))
        .collect()
}

struct Args {
    /// `--view`: it beats what was kept.
    view: Option<FileView>,
    admin: bool,
    rescan: bool,
    facet: Option<String>,
    socket: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    size: (u32, u32),
    help: bool,
}

fn parse(words: &[String]) -> Result<Args, String> {
    let mut out = Args {
        view: None,
        admin: false,
        rescan: false,
        facet: None,
        socket: None,
        snapshot: None,
        size: (1200, 800),
        help: false,
    };
    let mut it = words.iter();
    while let Some(word) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match word.as_str() {
            "--socket" => out.socket = Some(PathBuf::from(value("--socket")?)),
            "--snapshot" => out.snapshot = Some(PathBuf::from(value("--snapshot")?)),
            "--admin" => out.admin = true,
            "--rescan" => out.rescan = true,
            "--view" => {
                out.view = Some(match value("--view")?.as_str() {
                    "list" => FileView::List,
                    "grid" => FileView::Grid,
                    other => return Err(format!("--view wants list or grid, not {other}")),
                });
            }
            "--facet" => {
                let name = parse_facet(&value("--facet")?)?;
                if name == "admin" {
                    out.admin = true;
                } else {
                    fs_files_types::check_facet(&name)?;
                    out.admin = true;
                    out.facet = Some(name);
                }
            }
            "--size" => out.size = parse_size(&value("--size")?)?,
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(out)
}

/// Open the window. `words` are the words without the program name.
///
/// # Errors
/// The message for the person: bad words, no server, a picture that cannot be drawn.
pub fn run(words: &[String]) -> Result<(), String> {
    // The typing test reads what is typed into the fields: the registry must be on BEFORE the first field is made
    // (fields made earlier are not noted), so before the window is built. A debug-only feature, off in a release.
    #[cfg(feature = "test-access")]
    fs_components_widgets::test_access::start_from_env();
    let args = parse(words)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let socket = args
        .socket
        .clone()
        .or_else(|| default_socket("files"))
        .ok_or("no socket: give --socket (no default socket on this system)")?;
    // This window is a user of the server for as long as it lives: the shared lock on the lease is let go by the
    // system when the process ends. A server this window starts ends itself when no window holds the lease any more.
    hold_lease(&companion_path(&socket, "lease"));
    let remote = Arc::new(FilesRemote::matching(
        &socket,
        Arc::new(starter(&socket, args.socket.is_some())?),
    ));
    if args.rescan {
        use fs_components_workbenches::file_manager::FileSource;
        crate::RemoteFileSource(Arc::clone(&remote))
            .rescan(None)
            .map_err(|e| e.to_string())?;
    }
    let app = if args.admin {
        admin_app(&remote, args.facet.as_deref())?
    } else {
        // The view: the argument, else what the person chose last time (View menu, Face), else the list.
        let kept = settings::default_path();
        let view = args
            .view
            .unwrap_or_else(|| kept.as_deref().map_or(FileView::List, settings::load));
        files_widget(&remote, view, kept).map_err(|e| e.to_string())?
    };
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("wrote {}", file.display());
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}

/// The generated Admin frontend over `remote`.
fn admin_app(
    remote: &Arc<FilesRemote>,
    facet: Option<&str>,
) -> Result<fs_components_app::AppWidget, String> {
    // No server (a handshake that failed): the window still opens, with the descriptions this program has itself;
    // every list then says what is wrong and offers "Try again" (ADR 0065). Never a black window.
    let descriptions = remote
        .describe()
        .unwrap_or_else(|_| fs_files_types::registry().iter().cloned().collect());
    let descriptions = in_facet(descriptions, facet);
    app_widget(remote, descriptions).map_err(|e| e.to_string())
}

/// How this window starts the server when none fits: `fs-files serve --lease <lease>` (detached by `CommandStarter`, so
/// other windows use it too), on the socket this window talks to. The server ends itself when no window holds the lease
/// any more; one started by hand has none. What it says when it cannot start (a bad folder, a data folder others can
/// read) it says on its own stderr, which a detached server has not: the person sees the handshake error instead.
fn starter(socket: &Path, own_socket: bool) -> Result<CommandStarter, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut words: Vec<std::ffi::OsString> = vec!["serve".into()];
    if own_socket {
        words.extend(["--socket".into(), socket.as_os_str().to_owned()]);
    }
    words.extend([
        "--lease".into(),
        companion_path(socket, "lease").into_os_string(),
    ]);
    Ok(CommandStarter::new(me, words))
}

/// Hold the shared lock on `path` until this process ends.
fn hold_lease(path: &Path) {
    static HELD: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
    // Read and write, not append: Windows takes a lock only on a handle that can read or write data.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path);
    if let Ok(file) = file
        && file.lock_shared().is_ok()
    {
        let _ = HELD.set(file);
    }
}
