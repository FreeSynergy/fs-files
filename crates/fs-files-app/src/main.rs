//! The binary: wiring only.
//!
//! Pattern: Facade consumer — parse arguments, open the store and the folder, look at the folder, then
//! transfer or show. The runtime lives as long as the window: the workbench's calls run on it.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::file_manager::{FileManagerWidget, FileSource, PAGE};
use fs_files_app::args::{Args, HELP, parse};
use fs_files_app::{FileAdapter, commands, labels, screen_widget};
use fs_files_core::{FileRepo, LocalFolder};
use fs_store::{Store, TursoStore};

/// Who acts on this store. Provisional: there is no sign-in yet.
const ACTOR: &str = "https://localhost/actors/owner";
/// The name of the only backend. Provisional: more come with `backend.add`.
const BACKEND: &str = "local";

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
    let root = args.root.as_deref().ok_or("--root is needed")?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let source = open(&args, root, runtime.handle())?;
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

fn open(
    args: &Args,
    root: &Path,
    runtime: &tokio::runtime::Handle,
) -> Result<Arc<dyn FileSource>, String> {
    let store: Arc<dyn Store> = runtime
        .block_on(async {
            match &args.store {
                Some(file) => TursoStore::open(file).await,
                None => TursoStore::open_in_memory().await,
            }
        })
        .map(|s| Arc::new(s) as Arc<dyn Store>)
        .map_err(|e| format!("cannot open the store: {e}"))?;
    let folder = runtime
        .block_on(LocalFolder::open(root))
        .map_err(|e| format!("cannot open {}: {e}", root.display()))?;
    // One read per page of the workbench: each costs in proportion to the store (`DESIGN.md`).
    let repo = FileRepo::new(store, ACTOR, BACKEND, Arc::new(folder)).with_page_size(PAGE);
    Ok(Arc::new(FileAdapter::new(
        Arc::new(repo.clone()),
        Arc::new(repo),
        runtime.clone(),
    )))
}

/// The core keeps no watch and has no start hook: after a crash the program must look itself.
fn look_at_folder(source: &dyn FileSource) -> Status {
    let began = Instant::now();
    match source.rescan(None) {
        Ok(report) => {
            let line = format!(
                "Looked at the folder in {:.1} s: {} taken over, {} changed, {} missing, {} not taken over. Changes made outside appear after Rescan.",
                began.elapsed().as_secs_f32(),
                report.added.len(),
                report.updated.len(),
                report.missing.len(),
                report.skipped.len(),
            );
            eprintln!("{line}");
            Status::Info(line)
        }
        Err(e) => Status::Problem(format!("Could not look at the folder: {e}")),
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
