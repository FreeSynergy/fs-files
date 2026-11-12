//! A transfer that takes its time must not hold the window: the file workbench over `RemoteFileSource`.
//!
//! Pattern: none (tests) — a fake server (`files::Server`) whose `transfer.startUpload` never answers (a long upload)
//! and whose other calls answer at once (with an error: they are not implemented). The workbench starts the upload,
//! and during it the window must answer its input and a short call; at the limit the row ends as failed.

use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use capnp_rpc::new_client;
use fs_components_workbenches::file_manager::{FileLabels, FileManagerWidget};
use fs_files_app::{FilesRemote, Limits, RemoteFileSource};
use fs_interfaces::files_capnp::{backends, files, folders, search, transfer};
use fs_interfaces::rpc::{ServerIdentity, run_until_signal_as};

const LIMIT: Duration = Duration::from_millis(1500);

fn limits() -> Limits {
    Limits {
        start: LIMIT,
        call: LIMIT,
        long: LIMIT,
    }
}

/// Answers every accessor with a capability that knows no method, except `transfer.startUpload`, which never answers.
struct Quiet;
impl folders::Server for Quiet {}
impl transfer::Server for Quiet {
    async fn start_upload(
        self: Rc<Self>,
        _: transfer::StartUploadParams,
        _: transfer::StartUploadResults,
    ) -> Result<(), capnp::Error> {
        std::future::pending().await
    }
}
impl backends::Server for Quiet {}
impl search::Server for Quiet {}

/// The root.
struct Root;
impl files::Server for Root {
    fn folders(
        self: Rc<Self>,
        _: files::FoldersParams,
        mut r: files::FoldersResults,
    ) -> impl Future<Output = Result<(), capnp::Error>> {
        r.get().set_folders(new_client(Quiet));
        std::future::ready(Ok(()))
    }
    fn search(
        self: Rc<Self>,
        _: files::SearchParams,
        mut r: files::SearchResults,
    ) -> impl Future<Output = Result<(), capnp::Error>> {
        r.get().set_search(new_client(Quiet));
        std::future::ready(Ok(()))
    }
    fn transfer(
        self: Rc<Self>,
        _: files::TransferParams,
        mut r: files::TransferResults,
    ) -> impl Future<Output = Result<(), capnp::Error>> {
        r.get().set_transfer(new_client(Quiet));
        std::future::ready(Ok(()))
    }
    fn backends(
        self: Rc<Self>,
        _: files::BackendsParams,
        mut r: files::BackendsResults,
    ) -> impl Future<Output = Result<(), capnp::Error>> {
        r.get().set_backends(new_client(Quiet));
        std::future::ready(Ok(()))
    }
}

fn serve(socket: &Path) {
    let socket = socket.to_path_buf();
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let identity = ServerIdentity::new("fs-files", env!("CARGO_PKG_VERSION"));
        let _ = run_until_signal_as(&socket, identity, || async move {
            up.send(()).ok();
            let root: files::Client = new_client(Root);
            Ok(root.client)
        });
    });
    started.recv().unwrap();
}

fn remote(socket: &Path) -> Arc<FilesRemote> {
    let refuse = || -> std::io::Result<()> { Err(std::io::Error::other("must not be started")) };
    Arc::new(FilesRemote::matching_with(
        socket,
        Arc::new(refuse),
        limits(),
    ))
}

#[test]
fn a_long_upload_leaves_the_window_free_and_ends_at_its_limit() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("files.sock");
    serve(&socket);
    let file = dir.path().join("big.bin");
    std::fs::write(&file, b"bytes").unwrap();
    let source = Arc::new(RemoteFileSource(remote(&socket)));
    let widget = FileManagerWidget::new(source, FileLabels::default());
    let began = Instant::now();
    widget.with_state(|s| {
        s.begin_upload();
        s.set_input(file.to_str().unwrap());
        s.submit_input();
    });
    assert!(
        began.elapsed() < LIMIT / 3,
        "the start held the window for {:?}",
        began.elapsed()
    );
    assert!(widget.with_state(|s| s.is_running()));
    // The window answers: typing is taken, and a short call (the list) is answered by the server meanwhile.
    let asked = Instant::now();
    widget.with_state(|s| s.search("a"));
    assert!(
        asked.elapsed() < LIMIT / 3,
        "a short call waited {:?}",
        asked.elapsed()
    );
    assert_eq!(widget.with_state(|s| s.term().to_string()), "a");
    assert!(widget.with_state(|s| s.is_running()), "still on its way");
    // At the limit the call ends visibly: no row says done.
    let until = Instant::now() + LIMIT * 4;
    while widget.with_state(|s| s.is_running()) {
        assert!(Instant::now() < until, "the call never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    let rows = widget.with_state(|s| s.transfers().clone());
    assert_eq!(rows.len(), 1);
    assert!(matches!(
        rows[0].shown.state,
        fs_components_widgets::transfer_list::State::Failed(_)
    ));
}
