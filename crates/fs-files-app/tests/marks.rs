//! A marked file (ADR 0029), by the core's real field: marked wherever it appears, not read by itself,
//! written out only by an explicit act. A name proves nothing.
//!
//! Pattern: integration tests over the real core on an encrypted store (the store refuses marks without
//! a key, which is tested too).

mod common;

use common::World;
use fs_components_workbenches::file_manager::{
    Ask, CarryKind, Confirm, FileProblem, FileSource, Notice, PreviewContent,
};
use fs_files_app::labels;

#[test]
fn a_store_file_without_a_key_refuses_a_mark_and_shows_that_plainly() {
    let w = World::plain_file();
    let f = w.file(None, "a.txt", b"x");
    let refused = w.runtime.block_on(fs_files_core::Files::classify(
        &w.repo,
        f.id,
        vec!["dpv:Health".into()],
    ));
    assert!(refused.is_err());
    assert!(w.get(f.id).unwrap().sensitive.is_empty());
}

#[test]
fn the_mark_follows_the_file_to_list_search_trash_and_copy_and_a_name_proves_nothing() {
    let w = World::encrypted();
    let m = w.file(None, "report.txt", b"private words");
    let plain = w.file(None, "sensitive health secret.txt", b"not marked");
    w.mark(m.id);
    let mut s = w.state();
    let marked_of = |key: String, s: &fs_components_workbenches::file_manager::FilesState| {
        s.rows().iter().find(|r| r.key == key).unwrap().is_marked()
    };
    assert!(marked_of(m.id.to_string(), &s), "in the list");
    assert!(
        !marked_of(plain.id.to_string(), &s),
        "a name proves nothing"
    );
    s.search("report");
    assert!(marked_of(m.id.to_string(), &s), "in a search");
    s.search("");
    s.select(&m.id.to_string());
    s.carry(CarryKind::Copy);
    assert!(s.carrying().unwrap().entry.is_marked());
    s.put_down();
    assert!(
        matches!(s.ask(), Ask::NameTaken(_)),
        "same folder: the name is taken"
    );
    s.keep_both();
    let copy = s.rows().iter().find(|r| r.name.contains("copy")).unwrap();
    assert!(copy.is_marked(), "a copy stays marked");
    s.select(&m.id.to_string());
    s.ask_delete();
    assert!(
        s.confirm_words().unwrap().0.contains(&labels().marked),
        "named as marked"
    );
    s.really();
    s.open_trash();
    assert!(marked_of(m.id.to_string(), &s), "in the trash");
    s.ask_empty_trash();
    assert!(s.confirm_words().unwrap().0.contains("marked"));
}

#[test]
fn a_marked_file_is_not_read_until_asked_and_nothing_is_selected_by_itself() {
    let w = World::encrypted();
    let m = w.file(None, "a.txt", b"private words");
    w.mark(m.id);
    let mut s = w.state();
    assert_eq!(s.selected_key(), None);
    // A change outside would be refused if the file were read: it is not even opened.
    std::fs::write(w.root().join("a.txt"), b"changed").unwrap();
    s.select(&m.id.to_string());
    assert_eq!(
        s.content(),
        Some(&PreviewContent::Note(labels().preview_marked))
    );
    assert!(s.preview_held_back());
    s.rescan();
    s.select(&m.id.to_string());
    s.release_preview();
    match s.content() {
        Some(PreviewContent::Text(text, _)) => assert_eq!(text.lines, ["changed"]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_download_of_a_marked_file_needs_the_second_act_and_is_private() {
    let w = World::encrypted();
    let m = w.file(None, "a.txt", b"private words");
    w.mark(m.id);
    let target = w.outside().join("out.txt");
    let path = target.to_str().unwrap();
    // The source refuses by itself.
    let said = w
        .adapter
        .download(&m.id.to_string(), path, false)
        .unwrap_err();
    assert_eq!(said.problem, FileProblem::MarkedNeedsRelease);
    assert!(!target.exists());
    let mut s = w.state();
    s.select(&m.id.to_string());
    s.begin_download();
    s.set_input(path);
    s.submit_input();
    assert!(matches!(
        s.ask(),
        Ask::Confirm(Confirm::DownloadMarked { .. })
    ));
    assert!(!target.exists(), "only asked");
    s.really();
    assert_eq!(std::fs::read(&target).unwrap(), b"private words");
    assert!(matches!(s.notice(), Some(Notice::Info(_))));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "readable by its owner only");
    }
}

#[test]
fn the_command_line_needs_the_release_too_and_a_report_names_the_mark() {
    let w = World::encrypted();
    let m = w.file(None, "a.txt", b"private");
    w.mark(m.id);
    let target = w.outside().join("cli.txt");
    let path = target.to_str().unwrap();
    let key = m.id.to_string();
    assert!(fs_files_app::commands::download(w.adapter.as_ref(), &key, path, false).is_err());
    assert!(!target.exists());
    fs_files_app::commands::download(w.adapter.as_ref(), &key, path, true).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"private");
    std::fs::remove_file(w.root().join("a.txt")).unwrap();
    let report = w.adapter.rescan(None).unwrap();
    assert_eq!(report.missing, [format!("[{}] a.txt", labels().marked)]);
}
