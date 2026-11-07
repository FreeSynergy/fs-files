//! A taken name asks; keep both gets a free name; nothing is ever replaced.
//!
//! Pattern: integration tests over the real core and a real folder. Whatever is on the disk under the
//! taken name keeps its bytes.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{Ask, CarryKind, FilesState};

fn asked(s: &FilesState) -> String {
    match s.ask() {
        Ask::NameTaken(question) => question.message.clone(),
        other => panic!("no question: {other:?}"),
    }
}

#[test]
fn rename_onto_a_taken_name_asks_changes_nothing_and_keep_both_keeps_both() {
    let w = World::new();
    let a = w.file(None, "a.txt", b"bytes of a");
    w.file(None, "b.txt", b"bytes of b");
    let mut s = w.state();
    s.select(&a.id.to_string());
    s.begin_rename();
    s.set_input("b.txt");
    s.submit_input();
    assert!(asked(&s).contains("taken"));
    assert_eq!(w.get(a.id).unwrap().name, "a.txt");
    assert_eq!(w.tree(), ["a.txt", "b.txt"]);
    s.keep_both();
    assert_eq!(*s.ask(), Ask::Nothing);
    assert_eq!(w.tree(), ["b (2).txt", "b.txt"]);
    assert_eq!(
        std::fs::read(w.root().join("b.txt")).unwrap(),
        b"bytes of b"
    );
    assert_eq!(
        std::fs::read(w.root().join("b (2).txt")).unwrap(),
        b"bytes of a"
    );
}

#[test]
fn move_copy_and_upload_onto_a_taken_name_ask_and_replace_nothing() {
    let w = World::new();
    let d = w.folder(None, "d");
    let a = w.file(None, "a.txt", b"mine");
    w.file(Some(d.id), "a.txt", b"theirs");
    let mut s = w.state();
    s.select(&a.id.to_string());
    s.carry(CarryKind::Move);
    s.open_folder(Some(&d.id.to_string()));
    s.put_down();
    asked(&s);
    assert_eq!(w.get(a.id).unwrap().folder, None, "it did not move");
    s.name_taken_back();
    assert!(
        s.carrying().is_some(),
        "still carried: choose another folder"
    );
    s.stop_carrying();

    s.open_folder(None);
    s.select(&a.id.to_string());
    s.carry(CarryKind::Copy);
    s.open_folder(Some(&d.id.to_string()));
    s.put_down();
    asked(&s);
    s.keep_both();
    assert_eq!(w.tree(), ["a.txt", "d", "d/a (copy).txt", "d/a.txt"]);
    assert_eq!(std::fs::read(w.root().join("d/a.txt")).unwrap(), b"theirs");

    let path = w.outside().join("a.txt");
    std::fs::write(&path, b"incoming").unwrap();
    s.begin_upload();
    s.set_input(path.to_str().unwrap());
    s.submit_input();
    asked(&s);
    assert_eq!(std::fs::read(w.root().join("d/a.txt")).unwrap(), b"theirs");
    s.keep_both();
    assert_eq!(
        std::fs::read(w.root().join("d/a (2).txt")).unwrap(),
        b"incoming"
    );
    assert_eq!(std::fs::read(w.root().join("d/a.txt")).unwrap(), b"theirs");
}

#[test]
fn a_folder_onto_a_taken_name_asks_too() {
    let w = World::new();
    let one = w.folder(None, "one");
    w.folder(None, "two");
    w.file(Some(one.id), "keep.txt", b"inside");
    let mut s = w.state();
    s.select(&one.id.to_string());
    s.begin_rename();
    s.set_input("two");
    s.submit_input();
    asked(&s);
    s.keep_both();
    assert_eq!(w.tree(), ["two", "two (2)", "two (2)/keep.txt"]);
}

#[test]
fn moving_a_folder_into_itself_says_why_and_nothing_moves() {
    let w = World::new();
    let a = w.folder(None, "a");
    let b = w.folder(Some(a.id), "b");
    let mut s = w.state();
    s.select(&a.id.to_string());
    s.carry(CarryKind::Move);
    s.open_folder(Some(&b.id.to_string()));
    s.put_down();
    let said = s.notice().unwrap().text().to_string();
    assert!(said.contains("into itself"), "{said}");
    assert_eq!(*s.ask(), Ask::Nothing);
    assert_eq!(w.tree(), ["a", "a/b"]);
}
