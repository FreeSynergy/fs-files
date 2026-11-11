//! The version handshake of the window: a server of this version is used, a missing one is started by the starter, and
//! when none fits the person gets `BackendError::Handshake` (the window shows it with "Try again"), never a black window.
//!
//! Pattern: none (tests). Nothing here stops a server: `Control.stop` is one flag per process, so the test that does
//! lives in its own file (`handshake_mismatch.rs`).

mod support;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{FileBackend, FilesRemote};
use support::serve_as;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn places() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let (socket, root) = (dir.path().join("files.sock"), dir.path().join("files"));
    (dir, socket, root)
}

#[test]
fn a_server_of_this_version_is_used_as_it_is() {
    let (_dir, socket, root) = places();
    serve_as(&socket, &root, VERSION);
    let refuse = || -> io::Result<()> { Err(io::Error::other("must not be started")) };
    let remote = Arc::new(FilesRemote::matching(&socket, Arc::new(refuse)));
    assert!(
        FileBackend(remote)
            .list(&Values::new())
            .is_ok_and(|l| l.is_empty())
    );
}

#[test]
fn no_server_and_no_start_is_a_handshake_error_that_a_second_try_can_end() {
    let (_dir, socket, root) = places();
    let tried = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&tried);
    let refuse = move || -> io::Result<()> {
        flag.store(true, Ordering::SeqCst);
        Err(io::Error::other("no executable"))
    };
    let remote = Arc::new(FilesRemote::matching(&socket, Arc::new(refuse)));
    assert!(tried.load(Ordering::SeqCst), "the starter was asked");
    let backend = FileBackend(Arc::clone(&remote));
    match backend.list(&Values::new()) {
        Err(BackendError::Handshake(text)) => assert!(text.contains("no executable"), "{text}"),
        other => panic!("expected a handshake error, got {other:?}"),
    }
    assert!(matches!(
        backend.reconnect(),
        Err(BackendError::Handshake(_))
    ));
    // The person fixes it (here: the server is there now) and presses "Try again".
    serve_as(&socket, &root, VERSION);
    backend.reconnect().unwrap();
    assert!(backend.list(&Values::new()).is_ok_and(|l| l.is_empty()));
}
