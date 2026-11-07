//! The binary: wiring only.
//!
//! Pattern: Facade consumer — parse arguments, connect to the server, ask it to look at its folder, then
//! transfer or show. The runtime lives as long as the window: the workbench's calls run on it.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::file_manager::{FileManagerWidget, FileSource};
use fs_files_app::args::{Args, HELP, parse};
use fs_files_app::{FileAdapter, cli, commands, fill, labels, screen_widget};
use fs_files_client::RpcFiles;

fn main() {
    if let Err(message) = run() {
        eprintln!("fs-files-app: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse(&argv)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let socket = args.socket.as_deref().ok_or(cli::NEEDS_SOCKET)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let source = open(socket, runtime.handle())?;
    let mut status = None;
    if !args.no_rescan {
        status = Some(look_at_folder(source.as_ref()));
    }
    if !args.upload.is_empty() || args.download.is_some() {
        for line in commands::upload(
            source.as_ref(),
            &args.upload,
            args.into.as_deref(),
            args.keep_both,
        )? {
            eprintln!("{line}");
        }
        if let (Some(id), Some(to)) = (&args.download, &args.to) {
            let line = commands::download(source.as_ref(), id, to, args.release_sensitive)?;
            eprintln!("{line}");
        }
        if args.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&args, source, status)
}

fn open(socket: &Path, runtime: &tokio::runtime::Handle) -> Result<Arc<dyn FileSource>, String> {
    let client = RpcFiles::connect(socket).map_err(|e| {
        fill(
            cli::CANNOT_REACH,
            &[
                ("socket", &socket.display().to_string()),
                ("why", &e.to_string()),
            ],
        )
    })?;
    let client = Arc::new(client);
    Ok(Arc::new(FileAdapter::new(
        client.clone(),
        client,
        runtime.clone(),
    )))
}

/// The server keeps no watch and has no start hook: after a crash the program must look itself.
fn look_at_folder(source: &dyn FileSource) -> Status {
    let began = Instant::now();
    match source.rescan(None) {
        Ok(report) => {
            let seconds = format!("{:.1}", began.elapsed().as_secs_f32());
            let count = |n: usize| n.to_string();
            let line = fill(
                cli::LOOKED,
                &[
                    ("seconds", &seconds),
                    ("added", &count(report.added.len())),
                    ("updated", &count(report.updated.len())),
                    ("missing", &count(report.missing.len())),
                    ("skipped", &count(report.skipped.len())),
                ],
            );
            eprintln!("{line}");
            Status::Info(line)
        }
        Err(e) => Status::Problem(fill(cli::LOOK_FAILED, &[("why", &e.to_string())])),
    }
}

fn show(args: &Args, source: Arc<dyn FileSource>, status: Option<Status>) -> Result<(), String> {
    let widget = FileManagerWidget::new(source, labels());
    if let Some(view) = args.view {
        widget.with_state(|s| s.set_view(view));
    }
    let app = screen_widget(widget).map_err(|e| e.to_string())?;
    app.set_status(status);
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
