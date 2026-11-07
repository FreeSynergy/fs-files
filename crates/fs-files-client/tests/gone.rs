//! A server that is not there, or stops: an error with words, never a hang or a panic.

mod common;

use std::time::Duration;

use common::Served;
use fs_files_client::{RpcFiles, Timeouts};
use fs_files_core::{Error, Files};
use fs_store::ObjectId;

#[tokio::test(flavor = "multi_thread")]
async fn nobody_listens() {
    let dir = tempfile::tempdir().unwrap();
    let err = RpcFiles::connect(&dir.path().join("none.sock"))
        .err()
        .expect("refused");
    assert!(!err.to_string().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_stops_is_an_error_on_the_next_call() {
    let mut served = Served::start().await;
    let files = RpcFiles::connect_with(
        &served.socket,
        Timeouts {
            connect: Duration::from_secs(5),
            call: Duration::from_secs(5),
        },
    )
    .unwrap();
    assert!(files.get(ObjectId::new()).await.unwrap().is_none());
    served.stop();
    let err = files.get(ObjectId::new()).await.unwrap_err();
    assert!(matches!(err, Error::Backend(_)), "{err:?}");
    assert!(err.to_string().contains("files server"), "{err}");
}
