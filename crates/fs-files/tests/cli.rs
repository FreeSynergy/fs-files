//! The words of the command line that need no server: `--version` (the release smoke test) and `--facet`.
//!
//! Pattern: none (tests).

use std::process::Command;

fn fs_files(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fs-files"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn it_prints_its_version() {
    let out = fs_files(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        concat!("fs-files ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_short_flag_prints_the_same_version() {
    let out = fs_files(&["-V"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        concat!("fs-files ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_server_refuses_a_facet_this_program_does_not_have() {
    let out = fs_files(&["serve", "--facet", "calendar"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown facet calendar"));
}

#[cfg(feature = "gui")]
#[test]
fn the_window_refuses_a_facet_this_program_does_not_have_before_it_connects() {
    for bad in ["calendar", "Gallery", "-x"] {
        let out = fs_files(&["--facet", bad]);
        assert_eq!(out.status.code(), Some(1), "{bad}");
        assert!(!out.stderr.is_empty(), "{bad}");
    }
}
