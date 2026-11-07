//! The real `fs-files-app` binary against the real server: what it refuses, what it says when nobody
//! listens, and a transfer by command line through the socket. Headless: no window opens.
//!
//! Pattern: none — a test file; it names what it tests: the binary's wiring.

mod common;

use std::process::{Command, Output};

use common::World;

fn app(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fs-files-app"))
        .args(args)
        .output()
        .expect("the binary starts")
}

fn said(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn without_a_socket_it_stops_and_says_so() {
    let out = app(&[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(said(&out).contains("--socket"), "{}", said(&out));
}

#[test]
fn a_root_or_a_store_is_refused_and_the_server_is_named() {
    for option in ["--root", "--store"] {
        let out = app(&[option, "/x", "--socket", "/nobody.sock"]);
        assert_eq!(out.status.code(), Some(1), "{option}");
        assert!(said(&out).contains("fs-files-server"), "{}", said(&out));
    }
}

#[test]
fn nobody_listening_is_a_message_and_exit_code_1() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("none.sock");
    let out = app(&["--socket", socket.to_str().unwrap(), "--no-rescan"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        said(&out).contains("cannot reach the files server"),
        "{}",
        said(&out)
    );
}

#[test]
fn a_file_goes_up_and_comes_down_by_command_line_through_the_server() {
    let w = World::new();
    let socket = w.socket.to_str().unwrap().to_string();
    let source = w.outside().join("up.txt");
    std::fs::write(&source, b"through the wire").unwrap();
    let up = app(&[
        "--socket",
        &socket,
        "--no-rescan",
        "--upload",
        source.to_str().unwrap(),
    ]);
    assert_eq!(up.status.code(), Some(0), "{}", said(&up));
    let on_disk = std::fs::read(w.root().join("up.txt")).unwrap();
    assert_eq!(on_disk, b"through the wire");
    let page = w
        .runtime
        .block_on(fs_files_core::Files::list(&w.repo, None, None))
        .unwrap();
    let fs_files_core::Entry::File(file) = &page.items[0] else {
        panic!("{page:?}")
    };
    let target = w.outside().join("down.txt");
    let down = app(&[
        "--socket",
        &socket,
        "--no-rescan",
        "--download",
        &file.id.to_string(),
        "--to",
        target.to_str().unwrap(),
    ]);
    assert_eq!(down.status.code(), Some(0), "{}", said(&down));
    assert_eq!(std::fs::read(&target).unwrap(), b"through the wire");
}
