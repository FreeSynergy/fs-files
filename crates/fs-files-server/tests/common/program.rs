//! The real program as a child process: started, waited for, stopped with a signal.

#![allow(dead_code)] // each test file uses a different subset

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::{Caps, caps};
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::connect;

pub fn start(socket: &Path, data: &Path, root: Option<&Path>) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fs-files-server"));
    command.arg("--socket").arg(socket).arg("--data").arg(data);
    match root {
        Some(root) => command.env("FS_FILES_ROOT", root),
        None => command.env_remove("FS_FILES_ROOT"),
    };
    command.stderr(Stdio::null()).spawn().unwrap()
}

pub async fn wait_for(socket: &Path) {
    for _ in 0..200 {
        if tokio::net::UnixStream::connect(socket).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server did not come up");
}

pub fn terminate(mut child: Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
}

pub async fn connected(socket: &Path) -> (fs_interfaces::rpc::Connection<files::Client>, Caps) {
    let connection = connect::<files::Client>(socket).await.unwrap();
    let c = caps(&connection.client).await;
    (connection, c)
}
