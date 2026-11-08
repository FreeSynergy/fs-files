//! The window of `fs-files` (ADR 0063): wiring only.
//!
//! Pattern: Facade consumer — connect to the server (starting it if none answers on the user's own
//! socket), build the file manager, open the window or draw one picture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_components_app::{
    AppError, AppWidget, Element, Screen, Surface, WindowOptions, launch, parse_size,
};
use fs_components_workbenches::file_manager::{FileLabels, FileManagerWidget, FileSource};
use fs_interfaces::rpc::default_socket;

use crate::FilesRemote;

/// The title of the window.
pub const TITLE: &str = "FreeSynergy Files";

/// The help text.
pub const HELP: &str = "fs-files: the files window (without arguments it opens)

  --socket <path>    the files server to talk to (default: $XDG_RUNTIME_DIR/freesynergy/files.sock);
                     without it, a server is started if none answers; start it yourself with `fs-files serve`
  --snapshot <png>   draw a picture instead of opening a window
  --size <WxH>       window or picture size (default 1200x800)";

struct FilesScreen(FileManagerWidget);

impl Surface for FilesScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }
}

/// The shell around the file manager over `source`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(source: Arc<dyn FileSource>) -> Result<AppWidget, AppError> {
    let widget = FileManagerWidget::new(source, FileLabels::default());
    let screen = Screen::new("files", TITLE, Arc::new(FilesScreen(widget)));
    AppWidget::new(TITLE, vec![screen])
}

struct Args {
    socket: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    size: (u32, u32),
    help: bool,
}

fn parse(words: &[String]) -> Result<Args, String> {
    let mut out = Args {
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
    let args = parse(words)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let socket = args
        .socket
        .clone()
        .or_else(|| default_socket("files"))
        .ok_or("no socket: give --socket or set XDG_RUNTIME_DIR")?;
    let remote = connect_or_start(&socket, args.socket.is_none())?;
    let app = app_widget(Arc::new(remote)).map_err(|e| e.to_string())?;
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

/// Connect; if none answers on the user's own socket, start `fs-files serve` (detached, so other
/// windows use it too) and wait for it. A socket given with `--socket` is never answered by starting
/// anything.
fn connect_or_start(socket: &Path, may_start: bool) -> Result<FilesRemote, String> {
    let first = FilesRemote::connect(socket);
    if first.is_ok() || !may_start {
        return first;
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut server = std::process::Command::new(me);
    server
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut server, 0);
    server
        .spawn()
        .map_err(|e| format!("could not start the files server: {e}"))?;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(remote) = FilesRemote::connect(socket) {
            return Ok(remote);
        }
    }
    first
}
