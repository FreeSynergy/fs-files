//! The real program: started, used, stopped and started again; the files are still there under their
//! real names.

mod common;

use std::process::{Command, Stdio};

use common::program::{connected, is_up, start, terminate, wait_for};
use common::{list_all, mkdir, names, put_file, text_file};
use fs_interfaces::files_capnp::OnConflict;

#[tokio::test(flavor = "current_thread")]
async fn files_survive_a_restart_under_their_real_names_and_a_second_start_is_refused() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("files.sock");
            let data = dir.path().join("data");

            let first = start(&socket, &data, None);
            wait_for(&socket).await;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&data).unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o700, "the data is for this user alone");
            }

            let mut second = start(&socket, &data, None);
            assert_eq!(
                second.wait().unwrap().code(),
                Some(1),
                "a second server started"
            );
            assert!(is_up(&socket).await, "the second start touched the first");

            let (connection, c) = connected(&socket).await;
            let docs = mkdir(&c, None, "Docs").await.unwrap();
            text_file(&c, Some(docs), "hello.txt", "hello").await;
            drop((c, connection));

            terminate(first);
            assert!(!is_up(&socket).await, "the socket stayed behind");
            let on_disk = data.join("files/Docs/hello.txt");
            assert_eq!(
                std::fs::read(on_disk).unwrap(),
                b"hello",
                "under its real name"
            );

            let again = start(&socket, &data, None);
            wait_for(&socket).await;
            let (connection, c) = connected(&socket).await;
            let top = list_all(&c, None).await;
            let inside = list_all(&c, Some(docs)).await;
            let listed = (names(&top), names(&inside));
            drop((c, connection));
            terminate(again);
            assert_eq!(listed, (vec!["Docs"], vec!["hello.txt"]));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_root_and_the_instance_are_taken_from_the_environment_and_the_arguments() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let (socket, root) = (dir.path().join("files.sock"), dir.path().join("my-files"));
            let mut command = Command::new(env!("CARGO_BIN_EXE_fs-files"));
            command
                .arg("serve")
                .arg("--socket")
                .arg(&socket)
                .arg("--data")
                .arg(dir.path().join("data"))
                .args(["--instance", "files.example.org"])
                .env("FS_FILES_ROOT", &root)
                .stderr(Stdio::null());
            let server = command.spawn().unwrap();
            wait_for(&socket).await;
            let (connection, c) = connected(&socket).await;
            let file = put_file(&c, None, "x.txt", "", b"x", OnConflict::Refuse).await;
            drop((c, connection));
            // Stopped first: a failed assertion must not leave the server running.
            terminate(server);
            let iri = file.unwrap().envelope.iri.unwrap();
            assert!(iri.starts_with("https://files.example.org/files/"), "{iri}");
            assert_eq!(std::fs::read(root.join("x.txt")).unwrap(), b"x");
            assert!(
                !dir.path().join("data/files").exists(),
                "the default was not used"
            );
        })
        .await;
}

#[test]
fn it_refuses_an_instance_that_is_no_domain() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        "a/b",
        "x y",
        "user@example.org",
        "example.org:80",
        "-a.example",
    ] {
        let code = Command::new(env!("CARGO_BIN_EXE_fs-files"))
            .arg("serve")
            .arg("--socket")
            .arg(dir.path().join("x.sock"))
            .arg("--data")
            .arg(dir.path().join("data"))
            .args(["--instance", bad])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code();
        assert_eq!(code, Some(2), "{bad}");
    }
    assert!(!dir.path().join("data").exists(), "nothing was opened");
}

#[test]
fn it_refuses_bad_arguments_and_a_data_directory_others_can_read() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_fs-files"))
            .arg("serve")
            .args(args)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .code()
    };
    assert_eq!(run(&["--help"]), Some(0));
    assert_eq!(run(&["--nonsense"]), Some(2));
    assert_eq!(run(&["--socket"]), Some(2));
    assert_eq!(
        run(&["--socket", "/s", "--data", "/d", "--actor", ""]),
        Some(2)
    );

    // A data folder others can reach is refused: Unix modes (Windows makes the folder private itself, nothing to refuse).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let open = dir.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, PermissionsExt::from_mode(0o755)).unwrap();
        let socket = dir.path().join("x.sock");
        let (socket, open) = (socket.to_str().unwrap(), open.to_str().unwrap());
        assert_eq!(run(&["--socket", socket, "--data", open]), Some(1));
    }
}
