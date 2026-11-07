//! The command line.
//!
//! Pattern: table of inputs and answers.

use fs_components_workbenches::file_manager::FileView;
use fs_files_app::args::parse;

fn args(line: &[&str]) -> Vec<String> {
    line.iter().map(ToString::to_string).collect()
}

#[test]
fn what_the_other_apps_take_is_taken_here_too() {
    let a = parse(&args(&[
        "--socket",
        "/r.sock",
        "--snapshot",
        "/p.png",
        "--size",
        "800x600",
        "--view",
        "grid",
    ]))
    .unwrap();
    assert_eq!(a.socket.unwrap().to_str(), Some("/r.sock"));
    assert_eq!(a.snapshot.unwrap().to_str(), Some("/p.png"));
    assert_eq!(a.size, (800, 600));
    assert_eq!(a.view, Some(FileView::Grid));
}

#[test]
fn a_socket_is_needed_and_transfers_go_together() {
    assert!(parse(&args(&[])).unwrap_err().contains("--socket"));
    assert!(parse(&args(&["--help"])).unwrap().help);
    let root = ["--socket", "/r.sock"];
    assert!(parse(&args(&[&root[..], &["--download", "x"]].concat())).is_err());
    assert!(parse(&args(&[&root[..], &["--to", "x"]].concat())).is_err());
    assert!(parse(&args(&[&root[..], &["--release-sensitive"]].concat())).is_err());
    assert!(parse(&args(&[&root[..], &["--into", "x"]].concat())).is_err());
    assert!(parse(&args(&[&root[..], &["--view", "shelf"]].concat())).is_err());
    assert!(parse(&args(&[&root[..], &["--wat"]].concat())).is_err());
    let up = parse(&args(
        &[
            &root[..],
            &["--upload", "a", "--upload", "b", "--keep-both"],
        ]
        .concat(),
    ))
    .unwrap();
    assert_eq!(up.upload, ["a", "b"]);
    assert!(up.keep_both);
}

#[test]
fn the_app_keeps_no_files_and_says_where_they_are_kept() {
    for option in ["--root", "--store"] {
        let said = parse(&args(&["--socket", "/s.sock", option, "/x"])).unwrap_err();
        assert!(said.contains(option), "{said}");
        assert!(said.contains("fs-files-server"), "{said}");
    }
    assert!(
        parse(&args(&["--root", "/x"]))
            .unwrap_err()
            .contains("fs-files-server")
    );
}
