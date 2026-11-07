//! The real program refuses places that would put the database among the files, and keeps the
//! backends a client adds below the root.

mod common;

use std::process::{Command, Stdio};

use common::program::{connected, start, terminate, wait_for};
use common::{is, put_file};

const INVALID: &str = "https://freesynergy.net/problems/files/invalid";

fn code_of(
    socket: &std::path::Path,
    data: &std::path::Path,
    root: &std::path::Path,
) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_fs-files-server"))
        .arg("--socket")
        .arg(socket)
        .arg("--data")
        .arg(data)
        .env("FS_FILES_ROOT", root)
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .code()
}

#[test]
fn it_refuses_a_relative_root_before_it_opens_anything() {
    let dir = tempfile::tempdir().unwrap();
    let code = Command::new(env!("CARGO_BIN_EXE_fs-files-server"))
        .arg("--socket")
        .arg(dir.path().join("x.sock"))
        .arg("--data")
        .arg(dir.path().join("data"))
        .env("FS_FILES_ROOT", "relative/files")
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .code();
    assert_eq!(code, Some(2));
    assert!(!dir.path().join("data").exists(), "nothing was opened");
}

#[test]
fn it_refuses_a_root_that_holds_the_data_and_leaves_no_trace_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let hidden = dir.path().join("not-made/..");
    for root in [dir.path().to_path_buf(), data.clone(), hidden] {
        let code = code_of(&dir.path().join("x.sock"), &data, &root);
        assert_eq!(code, Some(1), "{}", root.display());
        assert!(
            !root.join(".fs-files").exists(),
            "{} was touched",
            root.display()
        );
        assert!(!data.join("objects.db").exists(), "the database was opened");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_backend_added_over_the_socket_stays_below_the_root() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let (socket, root) = (dir.path().join("files.sock"), dir.path().join("root"));
            let server = start(&socket, &dir.path().join("data"), Some(&root));
            wait_for(&socket).await;
            let (connection, c) = connected(&socket).await;
            let outside = dir.path().join("outside");
            let add = |path: String| {
                let config = serde_json::json!({ "id": "two", "kind": "local", "path": path });
                let mut call = c.backends.add_request();
                call.get().set_config(config.to_string().as_str());
                call.send().promise
            };
            let mut results = Vec::new();
            for path in [outside.to_str().unwrap(), "../outside", "/etc"] {
                results.push(add(path.to_string()).await.err());
            }
            let ok = add("second".into()).await.is_ok();
            let put = put_file(
                &c,
                None,
                "x.txt",
                "",
                b"x",
                fs_interfaces::files_capnp::OnConflict::Refuse,
            );
            let stored = put.await.is_ok();
            drop((c, connection));
            terminate(server);
            for error in results {
                is(&error.expect("refused"), INVALID, 422);
            }
            assert!(ok && stored);
            assert!(!outside.exists(), "nothing was made outside the root");
            assert!(root.join(".fs-files/backends/second").is_dir());
            assert!(
                !root.join("second").exists(),
                "not among the files of the main backend"
            );
        })
        .await;
}
