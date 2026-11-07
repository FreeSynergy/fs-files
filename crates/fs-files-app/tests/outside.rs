//! Changes from outside: only by rescan, and a file that changed is refused, not hidden.
//!
//! Pattern: integration tests over the real core and a real folder.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{
    FileProblem, FileSource, Notice, PreviewContent, Side,
};

#[test]
fn a_new_file_outside_shows_only_after_rescan_and_the_report_says_what_was_found() {
    let w = World::new();
    let mut s = w.state();
    std::fs::write(w.root().join("from outside.txt"), b"hello").unwrap();
    s.retry();
    assert!(s.rows().is_empty(), "nothing watches the folder");
    s.rescan();
    assert_eq!(s.rows().len(), 1);
    assert_eq!(s.rows()[0].name, "from outside.txt");
    assert_eq!(s.side(), Side::Report);
    assert_eq!(s.report().unwrap().added, ["from outside.txt"]);
}

#[test]
fn a_file_gone_outside_is_reported_and_its_record_is_kept() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"bytes");
    let mut s = w.state();
    std::fs::remove_file(w.root().join("a.txt")).unwrap();
    s.rescan();
    assert_eq!(s.report().unwrap().missing, ["a.txt"]);
    assert!(w.get(f.id).is_some(), "not deleted");
    assert_eq!(s.rows().len(), 1);
}

#[test]
fn a_file_changed_outside_is_refused_shown_as_such_and_rescan_clears_it() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"bytes");
    let mut s = w.state();
    std::fs::write(w.root().join("a.txt"), b"longer than it was").unwrap();
    s.select(&f.id.to_string());
    match s.content() {
        Some(PreviewContent::Refused(why)) => {
            assert!(why.contains("changed outside"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    let target = w.outside().join("out.txt");
    s.begin_download();
    s.set_input(target.to_str().unwrap());
    s.submit_input();
    assert!(!target.exists(), "no wrong data written");
    assert!(matches!(
        s.notice(),
        Some(Notice::Problem(FileProblem::ChangedOutside, _))
    ));
    s.discard_after_failure();
    s.rescan();
    assert_eq!(s.report().unwrap().updated, ["a.txt"]);
    assert!(
        matches!(s.content(), Some(PreviewContent::Text(..))),
        "readable again"
    );
}

#[test]
fn trash_of_a_file_changed_outside_changes_nothing_and_says_so() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"bytes");
    let mut s = w.state();
    s.select(&f.id.to_string());
    std::fs::write(w.root().join("a.txt"), b"other bytes now").unwrap();
    s.ask_delete();
    s.really();
    assert!(matches!(
        s.notice(),
        Some(Notice::Problem(FileProblem::ChangedOutside, _))
    ));
    assert_eq!(
        std::fs::read(w.root().join("a.txt")).unwrap(),
        b"other bytes now"
    );
    assert!(!w.get(f.id).unwrap().trashed);
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_root_is_not_followed_or_taken_over() {
    let w = World::new();
    let secret = w.outside().join("secret.txt");
    std::fs::write(&secret, b"secret").unwrap();
    std::os::unix::fs::symlink(&secret, w.root().join("link.txt")).unwrap();
    std::os::unix::fs::symlink(w.outside(), w.root().join("linkdir")).unwrap();
    let mut s = w.state();
    s.rescan();
    assert!(s.rows().is_empty());
    let skipped = s.report().unwrap().skipped.join(" ");
    assert!(
        skipped.contains("link.txt") && skipped.contains("linkdir"),
        "{skipped}"
    );
    assert_eq!(std::fs::read(&secret).unwrap(), b"secret");
}

#[test]
fn rescan_is_offered_and_capabilities_follow_the_core() {
    let w = World::new();
    let caps = w.adapter.capabilities();
    assert!(caps.rescan && caps.keep_both && caps.restore_to && caps.upload);
    assert!(
        !caps.type_descending,
        "the contract: by type is ascending only"
    );
    assert!(!caps.dates, "a file carries no date yet");
}
