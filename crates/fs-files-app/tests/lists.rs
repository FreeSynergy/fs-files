//! Sort, filter, search, paging, and one bad record, through the real server and client.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{FileView, SortKey};

fn names(s: &fs_components_workbenches::file_manager::FilesState) -> Vec<String> {
    s.rows().iter().map(|r| r.name.clone()).collect()
}

fn world() -> (World, fs_files_core::Folder) {
    let w = World::new();
    w.file(None, "b.txt", &[1; 300]);
    w.file(None, "a.png", b"\x89PNG\r\n\x1a\n");
    w.file(None, "c.txt", &[1; 20]);
    let d = w.folder(None, "deep");
    w.file(Some(d.id), "big deep.txt", &[1; 5000]);
    (w, d)
}

#[test]
fn sort_by_name_size_type_and_direction_and_type_cannot_turn() {
    let (w, _) = world();
    let mut s = w.state();
    assert_eq!(names(&s), ["a.png", "b.txt", "c.txt", "deep"]);
    s.toggle_direction();
    assert_eq!(names(&s), ["deep", "c.txt", "b.txt", "a.png"]);
    s.set_sort(SortKey::Size);
    assert_eq!(names(&s)[0], "b.txt", "largest first");
    s.toggle_direction();
    assert_eq!(names(&s)[0], "deep", "a folder counts as 0");
    s.set_sort(SortKey::Type);
    assert!(!s.can_toggle_direction());
    assert!(!s.sort().descending);
    assert_eq!(s.list_problem(), None, "{:?}", s.list_problem());
    s.set_sort(SortKey::Modified);
    assert_eq!(s.list_problem(), None);
    assert_eq!(s.rows().len(), 4);
}

#[test]
fn a_filter_at_the_top_stays_at_the_top_and_in_a_folder_stays_in_it() {
    let (w, d) = world();
    let mut s = w.state();
    s.set_filter_field("min", "100");
    s.apply_filter();
    assert_eq!(s.list_problem(), None, "{:?}", s.list_problem());
    assert_eq!(names(&s), ["b.txt"], "big deep.txt lies deeper");
    s.open_folder(Some(&d.id.to_string()));
    assert_eq!(names(&s), ["big deep.txt"]);
    s.open_folder(None);
    s.clear_filter();
    s.set_filter_field("from", "2000-01-01");
    s.apply_filter();
    assert_eq!(s.rows().len(), 4);
    s.set_filter_field("before", "2000-01-02");
    s.apply_filter();
    assert!(s.rows().is_empty());
}

#[test]
fn search_finds_in_all_folders_by_part_of_the_name_and_leaves_with_a_folder() {
    let (w, d) = world();
    let mut s = w.state();
    s.search("deep");
    assert_eq!(names(&s), ["big deep.txt", "deep"]);
    s.open_folder(Some(&d.id.to_string()));
    assert_eq!(s.term(), "");
    assert_eq!(names(&s), ["big deep.txt"]);
}

#[test]
fn many_files_page_and_both_views_hold_the_same_rows() {
    let w = World::new();
    for i in 0..450 {
        std::fs::write(w.root().join(format!("f{i:04}.txt")), b"x").unwrap();
    }
    let mut s = w.state();
    s.rescan();
    assert_eq!(s.report().unwrap().added.len(), 450);
    assert_eq!(s.rows().len(), 200);
    assert!(s.has_more());
    s.load_more();
    s.load_more();
    assert_eq!(s.rows().len(), 450);
    assert!(!s.has_more());
    s.set_view(FileView::Grid);
    assert_eq!(s.rows().len(), 450);
}

#[test]
fn folders_for_the_tree_are_found_among_many_files_and_empty_files() {
    let w = World::new();
    let a = w.folder(None, "zzz folder last by name");
    let b = w.folder(None, "aaa");
    w.file(None, "empty.txt", b"");
    w.file(None, "full.txt", b"xx");
    let s = w.state();
    let top = &s.tree_nodes()[0];
    let keys: Vec<&str> = top.children.iter().map(|n| n.key.as_str()).collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(keys.contains(&a.id.to_string().as_str()));
    assert!(keys.contains(&b.id.to_string().as_str()));
}

/// The core reads a page whole: one record that cannot be read fails the page. The contract's `Page`
/// has no way to say "all but this one", so the view cannot name it and keep the rest of that page: it
/// keeps what is drawn, says why and offers to try again. Listed as a need of the core.
#[test]
fn one_broken_record_fails_the_page_of_the_core_and_the_gui_keeps_rows_and_says_why() {
    let w = World::new();
    let a = w.file(None, "a.txt", b"1");
    let b = w.file(None, "b.txt", b"2");
    let mut s = w.state();
    assert_eq!(s.rows().len(), 2);
    w.runtime.block_on(async {
        let mut object = w.store.get(b.id).await.unwrap().unwrap();
        object.content = serde_like_broken();
        w.store.put(object, common::ME).await.unwrap();
    });
    s.retry();
    assert_eq!(s.rows().len(), 2, "what was drawn stays");
    assert!(s.list_problem().is_some(), "and it says why");
    assert_eq!(std::fs::read(w.root().join("a.txt")).unwrap(), b"1");
    let _ = a;
}

fn serde_like_broken() -> serde_json::Value {
    serde_json::Value::String("not a file record".into())
}
