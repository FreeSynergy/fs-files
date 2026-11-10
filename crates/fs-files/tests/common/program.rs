//! The real program as a child process: started, waited for, stopped in order. Unix: with SIGTERM. Windows has no signal to send
//! to a program without a console, so the test holds the lease of the server (the way a window does) and lets go of it: the
//! server ends itself in order, as it does when its last window closes.

#![allow(dead_code)] // each test file uses a different subset

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::{Caps, caps};
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::connect;

pub fn start(socket: &Path, data: &Path, root: Option<&Path>) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fs-files"));
    command
        .arg("serve")
        .arg("--socket")
        .arg(socket)
        .arg("--data")
        .arg(data);
    match root {
        Some(root) => command.env("FS_FILES_ROOT", root),
        None => command.env_remove("FS_FILES_ROOT"),
    };
    spawn(&mut command, socket)
}

/// Starts the server of `command` (already `serve ...`) so that [`terminate`] can end it in order: on Windows the test holds
/// the lease of the server, as a window does.
pub fn spawn(command: &mut Command, socket: &Path) -> Child {
    #[cfg(windows)]
    let held = {
        let lease = socket.with_extension("lease");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lease)
            .unwrap();
        held.lock_shared().unwrap();
        command
            .arg("--lease")
            .arg(&lease)
            .env("FS_LEASE_POLL_MS", "100");
        held
    };
    #[cfg(unix)]
    let _ = socket;
    let child = command.stderr(Stdio::null()).spawn().unwrap();
    #[cfg(windows)]
    LEASES
        .lock()
        .unwrap()
        .push((child.id(), socket.to_owned(), held));
    child
}

/// The leases the tests of this process hold on the servers they started (Windows only needs them).
#[cfg(windows)]
static LEASES: std::sync::Mutex<Vec<(u32, std::path::PathBuf, std::fs::File)>> =
    std::sync::Mutex::new(Vec::new());

/// True while a server answers on `socket`.
pub async fn is_up(socket: &Path) -> bool {
    connect::<files::Client>(socket).await.is_ok()
}

pub async fn wait_for(socket: &Path) {
    for _ in 0..200 {
        if is_up(socket).await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the server did not come up");
}

#[cfg(unix)]
pub fn terminate(mut child: Child) {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
}

#[cfg(windows)]
pub fn terminate(mut child: Child) {
    // Letting go of the lease is what ends the server in order: it looks every 100 ms and ends after three empty looks.
    // Every lease on the socket of this server is let go of: a second server that was refused (and ended at once) also took one,
    // and a lease that nobody ever lets go of would keep the first server alive.
    let mut leases = LEASES.lock().unwrap();
    let socket = leases
        .iter()
        .find(|(pid, _, _)| *pid == child.id())
        .map(|(_, socket, _)| socket.clone());
    let held = socket.is_some();
    leases.retain(|(_, s, _)| Some(s) != socket.as_ref());
    drop(leases);
    if !held {
        // A server that was not started through `spawn` has no lease to let go of: never wait for ever.
        let _ = child.kill();
        panic!("terminate was given a server that holds no lease");
    }
    assert!(child.wait().unwrap().success());
}

pub async fn connected(socket: &Path) -> (fs_interfaces::rpc::Connection<files::Client>, Caps) {
    let connection = connect::<files::Client>(socket).await.unwrap();
    let c = caps(&connection.client).await;
    (connection, c)
}
