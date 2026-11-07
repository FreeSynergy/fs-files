//! The trash: restore, restore to a place, purge, empty; and what is gone is really gone.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{Ask, CarryKind, Confirm};

#[test]
fn delete_needs_the_second_act_then_restore_brings_the_bytes_back() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"bytes");
    let mut s = w.state();
    s.select(&f.id.to_string());
    s.ask_delete();
    s.ask_delete();
    assert_eq!(w.tree(), ["a.txt"], "pressing Delete again only asks again");
    s.really();
    assert!(w.tree().is_empty());
    assert!(w.get(f.id).unwrap().trashed);
    assert!(s.rows().is_empty());
    assert_eq!(s.selected_key(), None);
    s.open_trash();
    assert_eq!(s.rows().len(), 1);
    assert!(s.rows()[0].trashed);
    s.select(&f.id.to_string());
    s.restore_selected();
    assert_eq!(std::fs::read(w.root().join("a.txt")).unwrap(), b"bytes");
    assert!(s.rows().is_empty(), "it left the trash");
}

#[test]
fn restore_onto_a_taken_name_asks_and_keep_both_and_restore_to_a_place_work() {
    let w = World::new();
    let d = w.folder(None, "d");
    let f = w.file(None, "a.txt", b"old");
    let g = w.file(None, "b.txt", b"other");
    w.runtime.block_on(async {
        use fs_files_core::Files;
        w.repo.trash(f.id).await.unwrap();
        w.repo.trash(g.id).await.unwrap();
    });
    w.file(None, "a.txt", b"new");
    let mut s = w.state();
    s.open_trash();
    s.select(&f.id.to_string());
    s.restore_selected();
    assert!(matches!(s.ask(), Ask::NameTaken(_)));
    assert_eq!(std::fs::read(w.root().join("a.txt")).unwrap(), b"new");
    assert!(w.get(f.id).unwrap().trashed, "still in the trash");
    s.keep_both();
    assert_eq!(std::fs::read(w.root().join("a (2).txt")).unwrap(), b"old");
    assert_eq!(std::fs::read(w.root().join("a.txt")).unwrap(), b"new");

    s.open_trash();
    s.select(&g.id.to_string());
    s.carry(CarryKind::Restore);
    s.open_folder(Some(&d.id.to_string()));
    s.put_down();
    assert_eq!(std::fs::read(w.root().join("d/b.txt")).unwrap(), b"other");
    assert!(!w.get(g.id).unwrap().trashed);
}

#[test]
fn purge_and_empty_trash_say_what_goes_and_what_is_gone_is_gone_from_disk() {
    let w = World::new();
    let a = w.file(None, "a.txt", b"1");
    let b = w.file(None, "b.txt", b"2");
    let c = w.file(None, "c.txt", b"3");
    let keep = w.file(None, "keep.txt", b"4");
    w.runtime.block_on(async {
        use fs_files_core::Files;
        for id in [a.id, b.id, c.id] {
            w.repo.trash(id).await.unwrap();
        }
    });
    let mut s = w.state();
    s.open_trash();
    s.select(&a.id.to_string());
    s.ask_delete();
    assert!(matches!(s.ask(), Ask::Confirm(Confirm::Purge { .. })));
    assert!(s.confirm_words().unwrap().0.contains("a.txt"));
    s.really();
    assert_eq!(w.get(a.id), None);
    s.ask_empty_trash();
    let words = s.confirm_words().unwrap().0;
    assert!(words.contains("all 2 files"), "says how many: {words}");
    s.keep();
    assert!(w.get(b.id).is_some());
    s.ask_empty_trash();
    s.really();
    assert_eq!(w.get(b.id), None);
    assert_eq!(w.get(c.id), None);
    assert!(s.notice().unwrap().text().contains('2'));
    assert!(s.rows().is_empty());
    assert!(w.get(keep.id).is_some(), "what is not in the trash stays");
    assert_eq!(w.tree(), ["keep.txt"]);
    let trash = w.root().join(".fs-files/trash");
    let left = std::fs::read_dir(&trash).map_or(0, Iterator::count);
    assert_eq!(left, 0, "the bytes went");
    s.ask_empty_trash();
    assert_eq!(*s.ask(), Ask::Nothing, "an empty trash does not ask");
}

#[test]
fn a_thing_gone_behind_the_back_leaves_the_view() {
    let w = World::new();
    let f = w.file(None, "a.txt", b"1");
    let other = w.file(None, "b.txt", b"2");
    let mut s = w.state();
    s.select(&f.id.to_string());
    w.runtime.block_on(async {
        use fs_files_core::Files;
        w.repo.trash(f.id).await.unwrap();
        w.repo.purge(f.id).await.unwrap();
    });
    s.ask_delete();
    s.really();
    assert!(s.rows().iter().all(|r| r.key != f.id.to_string()));
    assert_eq!(s.selected_key(), None);
    assert!(s.notice().is_some());
    assert!(s.rows().iter().any(|r| r.key == other.id.to_string()));
}
