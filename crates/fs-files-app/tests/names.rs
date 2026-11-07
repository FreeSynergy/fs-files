//! Names, through the workbench's state and the real core, onto a real folder.
//!
//! Pattern: integration tests. A name reaches the backend only through the core's own check.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{Ask, FileEntry, FilesState};

fn rename_to(state: &mut FilesState, key: &str, name: &str) -> Option<String> {
    state.select(key);
    state.begin_rename();
    state.set_input(name);
    state.submit_input();
    match state.ask() {
        Ask::Input(input) => input.error.clone(),
        _ => None,
    }
}

fn row<'a>(rows: &'a [FileEntry], name: &str) -> &'a FileEntry {
    rows.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
}

#[test]
fn every_name_the_backend_can_hold_is_kept_exactly_and_is_the_real_name_on_disk() {
    let w = World::new();
    let file = w.file(None, "start.txt", b"bytes");
    let mut s = w.state();
    let key = file.id.to_string();
    let long = format!("{}.txt", "x".repeat(251));
    for name in [
        "with spaces in it.txt",
        "Übergrößenträger – 日本語 🦀.txt",
        "..x",
        "name.with.dots",
        " leading space",
        long.as_str(),
    ] {
        assert_eq!(rename_to(&mut s, &key, name), None, "{name}");
        assert_eq!(*s.ask(), Ask::Nothing, "{name}");
        assert_eq!(row(s.rows(), name).key, key);
        assert_eq!(
            w.get(file.id).unwrap().name,
            name,
            "envelope name is the real name"
        );
        assert_eq!(w.tree(), [name.to_string()], "on the disk under that name");
        s.select(&key);
    }
    assert_eq!(std::fs::read(w.root().join(&long)).unwrap(), b"bytes");
}

#[test]
fn a_name_that_looks_like_a_path_or_cannot_be_held_is_said_and_nothing_moves() {
    let w = World::new();
    let file = w.file(None, "start.txt", b"bytes");
    let mut s = w.state();
    let key = file.id.to_string();
    let outside = w.dir.path().join("x");
    let too_long = "y".repeat(256);
    for name in [
        "../x",
        "a/b",
        "/etc/passwd",
        "..",
        ".",
        "",
        "   ",
        "a\\b",
        "bad\nname",
        ".fs-files",
        too_long.as_str(),
    ] {
        let error =
            rename_to(&mut s, &key, name).unwrap_or_else(|| panic!("{name:?} was accepted"));
        assert!(
            error.contains("cannot be used"),
            "{name:?}: shown as invalid-name, was: {error}"
        );
        match s.ask() {
            Ask::Input(input) => assert_eq!(input.value, name, "what was typed stays"),
            other => panic!("{other:?}"),
        }
        s.discard_after_failure();
        assert_eq!(w.get(file.id).unwrap().name, "start.txt");
        assert_eq!(w.tree(), ["start.txt"], "{name:?}");
        assert!(!outside.exists());
    }
}

#[test]
fn a_new_folder_with_a_taken_or_bad_name_keeps_the_input_and_says_which() {
    let w = World::new();
    w.folder(None, "taken");
    let mut s = w.state();
    s.begin_new_folder();
    s.set_input("taken");
    s.submit_input();
    match s.ask() {
        Ask::Input(input) => assert!(input.error.as_deref().unwrap().contains("name is taken")),
        other => panic!("{other:?}"),
    }
    s.set_input("../up");
    s.submit_input();
    match s.ask() {
        Ask::Input(input) => assert!(input.error.as_deref().unwrap().contains("cannot be used")),
        other => panic!("{other:?}"),
    }
    assert!(!w.dir.path().join("up").exists());
    s.set_input("Neuer Ordner é");
    s.submit_input();
    assert_eq!(*s.ask(), Ask::Nothing);
    assert!(w.root().join("Neuer Ordner é").is_dir());
    assert!(s.rows().iter().any(|r| r.name == "Neuer Ordner é"));
}

#[test]
fn folder_tree_and_breadcrumb_follow_create_rename_move_and_delete() {
    let w = World::new();
    let a = w.folder(None, "a");
    let b = w.folder(Some(a.id), "b");
    let mut s = w.state();
    s.open_folder(Some(&b.id.to_string()));
    assert_eq!(
        s.path().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    let top = &s.tree_nodes()[0];
    assert_eq!(top.children[0].label, "a");
    assert_eq!(top.children[0].children[0].label, "b");
    s.open_folder(Some(&a.id.to_string()));
    s.select(&b.id.to_string());
    s.begin_rename();
    s.set_input("c");
    s.submit_input();
    assert!(w.root().join("a/c").is_dir());
    assert_eq!(s.tree_nodes()[0].children[0].children[0].label, "c");
    s.ask_delete();
    s.really();
    assert!(!w.root().join("a/c").exists());
    assert!(s.rows().is_empty(), "it left the list");
}
