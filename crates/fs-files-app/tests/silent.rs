//! A server that passes the handshake and then says nothing: the window must not wait for ever.
//!
//! Pattern: none (tests) — a fake server (`files::Server`) answers `Control.version` (through `run_until_signal_as`) and
//! then keeps silent, once on the first questions after the handshake, once on a call from the window. Own file: the
//! servers are never stopped, they end with the process.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use capnp_rpc::new_client;
use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{FileBackend, FilesRemote, Limits};
use fs_interfaces::files_capnp::{backends, files, folders, search, transfer};
use fs_interfaces::rpc::{ServerIdentity, run_until_signal_as};

const LIMIT: Duration = Duration::from_millis(400);

fn limits() -> Limits {
    Limits {
        start: LIMIT,
        call: LIMIT,
        long: LIMIT * 2,
    }
}

/// Answers every accessor with a capability that knows no method, except `search.find`, which never answers.
struct Quiet;
impl folders::Server for Quiet {}
impl transfer::Server for Quiet {}
impl backends::Server for Quiet {}
impl search::Server for Quiet {
    async fn find(
        self: Rc<Self>,
        _: search::FindParams,
        _: search::FindResults,
    ) -> Result<(), capnp::Error> {
        std::future::pending().await
    }
}

/// The root: with `hang_start` the first accessor of the start (`folders`) never answers.
struct Root {
    hang_start: bool,
}
impl files::Server for Root {
    async fn folders(
        self: Rc<Self>,
        _: files::FoldersParams,
        mut r: files::FoldersResults,
    ) -> Result<(), capnp::Error> {
        if self.hang_start {
            std::future::pending::<()>().await;
        }
        r.get().set_folders(new_client(Quiet));
        Ok(())
    }
    async fn search(
        self: Rc<Self>,
        _: files::SearchParams,
        mut r: files::SearchResults,
    ) -> Result<(), capnp::Error> {
        r.get().set_search(new_client(Quiet));
        Ok(())
    }
    async fn transfer(
        self: Rc<Self>,
        _: files::TransferParams,
        mut r: files::TransferResults,
    ) -> Result<(), capnp::Error> {
        r.get().set_transfer(new_client(Quiet));
        Ok(())
    }
    async fn backends(
        self: Rc<Self>,
        _: files::BackendsParams,
        mut r: files::BackendsResults,
    ) -> Result<(), capnp::Error> {
        r.get().set_backends(new_client(Quiet));
        Ok(())
    }
}

fn serve(socket: &Path, hang_start: bool) {
    let socket = socket.to_path_buf();
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let identity = ServerIdentity::new("fs-files", env!("CARGO_PKG_VERSION"));
        let _ = run_until_signal_as(&socket, identity, || async move {
            up.send(()).ok();
            let root: files::Client = new_client(Root { hang_start });
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

/// The text of the error and how long it took; fails if the error is no visible one.
fn handshake_text(error: BackendError) -> String {
    match error {
        BackendError::Handshake(text) => text,
        other => panic!("expected a visible error with \"Try again\", got {other:?}"),
    }
}

#[test]
fn a_server_that_is_silent_after_the_handshake_does_not_keep_the_window_from_opening() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("files.sock");
    serve(&socket, true);
    let began = Instant::now();
    let remote = remote(&socket);
    assert!(
        began.elapsed() < LIMIT * 10,
        "the start waited {:?}",
        began.elapsed()
    );
    let text = handshake_text(FileBackend(remote).list(&Values::new()).unwrap_err());
    assert!(text.contains("did not answer"), "{text}");
}

#[test]
fn a_call_that_the_server_never_answers_ends_with_the_limit_and_the_connection_stays_usable() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("files.sock");
    serve(&socket, false);
    let remote = remote(&socket);
    let backend = FileBackend(remote);
    for _ in 0..2 {
        let began = Instant::now();
        let text = handshake_text(backend.list(&Values::new()).unwrap_err());
        let took = began.elapsed();
        assert!(text.contains("did not answer"), "{text}");
        assert!(took >= LIMIT, "too early: {took:?}");
        assert!(took < LIMIT * 5, "the limit did not hold: {took:?}");
    }
}
