//! A server of another version that cannot be replaced: the window gets the words of `HandshakeError`.
//!
//! Pattern: none (tests). Own file: the handshake stops the old server (`Control.stop`), one flag per process.

mod support;

use std::io;
use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{FileBackend, FilesRemote};
use support::serve_as;

#[test]
fn an_old_server_that_cannot_be_replaced_is_reported_with_both_versions() {
    let dir = tempfile::tempdir().unwrap();
    let (socket, root) = (dir.path().join("files.sock"), dir.path().join("files"));
    serve_as(&socket, &root, "0.0.1");
    let refuse = || -> io::Result<()> { Err(io::Error::other("cannot start here")) };
    let remote = Arc::new(FilesRemote::matching(&socket, Arc::new(refuse)));
    match FileBackend(remote).list(&Values::new()) {
        Err(BackendError::Handshake(text)) => {
            assert!(text.contains("0.0.1"), "what runs: {text}");
            assert!(
                text.contains(env!("CARGO_PKG_VERSION")),
                "what is needed: {text}"
            );
        }
        other => panic!("expected a handshake error, got {other:?}"),
    }
}
