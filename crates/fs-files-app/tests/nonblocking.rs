//! A transfer that takes its time must not hold the window: the file workbench over `RemoteFileSource`.
//!
//! Pattern: none (tests) — a fake server (`files::Server`) whose `transfer.startUpload` never answers (a long upload)
//! and whose other calls answer at once (with an error: they are not implemented). The workbench starts the upload,
//! and during it the window must answer its input and a short call; at the limit the row ends as failed.

use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
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

/// Answers every accessor with a capability that knows no method.
struct Quiet;
impl folders::Server for Quiet {}
/// The transfer: `startUpload` says that it arrived and then never answers (a long upload).
struct Slow(Sender<()>);
impl transfer::Server for Slow {
    async fn start_upload(
        self: Rc<Self>,
        _: transfer::StartUploadParams,
        _: transfer::StartUploadResults,
    ) -> Result<(), capnp::Error> {
        let _ = self.0.send(());
        std::future::pending().await
    }

    /// One unfinished upload, so that the window has a row to go on with.
    fn pending_uploads(
        self: Rc<Self>,
        _: transfer::PendingUploadsParams,
        mut r: transfer::PendingUploadsResults,
    ) -> impl Future<Output = Result<(), capnp::Error>> {
        let mut item = r.get().init_items(1).get(0);
        item.set_id("u1");
        item.set_name("big.iso");
        item.set_received(10);
        std::future::ready(Ok(()))
    }
}
impl backends::Server for Quiet {}
impl search::Server for Quiet {}

/// The root.
struct Root(Sender<()>);
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
        r.get().set_transfer(new_client(Slow(self.0.clone())));
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

fn serve(socket: &Path) -> Receiver<()> {
    let (arrived, upload_arrived) = std::sync::mpsc::channel();
    let socket = socket.to_path_buf();
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let identity = ServerIdentity::new("fs-files", env!("CARGO_PKG_VERSION"));
        let _ = run_until_signal_as(&socket, identity, || async move {
            up.send(()).ok();
            let root: files::Client = new_client(Root(arrived));
            Ok(root.client)
        });
    });
    started.recv().unwrap();
    upload_arrived
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
    let upload_arrived = serve(&socket);
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
    // The server has the upload and holds it: nothing below is a race with the worker.
    upload_arrived
        .recv_timeout(Duration::from_secs(5))
        .expect("the upload never reached the server");
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
    // (The server also lists one unfinished upload of its own; that row is not this call's.)
    let rows = widget.with_state(|s| s.transfers().clone());
    let own: Vec<_> = rows.iter().filter(|r| r.pending.is_none()).collect();
    assert_eq!(own.len(), 1);
    assert!(matches!(
        own[0].shown.state,
        fs_components_widgets::transfer_list::State::Failed(_)
    ));
}

#[test]
fn going_on_with_an_unfinished_upload_is_not_possible_yet_and_the_input_comes_back_with_the_reason()
{
    use fs_components_widgets::transfer_list::{Action, Outcome};
    use fs_components_workbenches::file_manager::Ask;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("files.sock");
    let _arrived = serve(&socket);
    let source = Arc::new(RemoteFileSource(remote(&socket)));
    let widget = FileManagerWidget::new(source, FileLabels::default());
    widget.with_state(|s| {
        s.open_transfers();
        let key = s.transfers()[0].shown.key.clone();
        s.transfer_outcome(&Outcome::new(key, Action::Retry));
        assert!(matches!(s.ask(), Ask::Input(_)), "asks for the path first");
        s.set_input("/tmp/big.iso");
        s.submit_input();
    });
    let until = Instant::now() + Duration::from_secs(5);
    while widget.with_state(|s| s.is_running()) {
        assert!(Instant::now() < until, "the answer never came in");
        std::thread::sleep(Duration::from_millis(10));
    }
    widget.with_state(|s| match s.ask() {
        Ask::Input(i) => {
            assert_eq!(i.value, "/tmp/big.iso", "what was typed is kept");
            assert!(
                i.error
                    .as_deref()
                    .is_some_and(|e| e.contains("not possible yet")),
                "{:?}",
                i.error
            );
        }
        other => panic!("the input did not come back: {other:?}"),
    });
}
