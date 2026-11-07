//! The menus: only real commands, and an entry really changes the workbench's state.
//!
//! Pattern: Command tests over the real core, through the shell's own way of choosing an entry.

mod common;

use common::World;
use fs_components_workbenches::file_manager::{Ask, FileView, Side};
use fs_files_app::screen_widget;

#[test]
fn the_entries_are_real_and_nothing_destructive_is_among_them() {
    let w = World::new();
    let widget = w.widget();
    let app = screen_widget(widget.clone()).unwrap();
    let labels: Vec<String> = app.with_shell(|shell| {
        shell
            .menus()
            .iter()
            .flat_map(|menu| {
                let lines = menu.entries.iter().map(|e| e.label.clone());
                std::iter::once(menu.label.clone()).chain(lines)
            })
            .collect()
    });
    assert_eq!(
        labels,
        [
            "File",
            "New folder",
            "Upload",
            "Rescan",
            "View",
            "List",
            "Grid",
            "Trash",
            "Transfers"
        ]
    );
    assert!(app.choose("view-grid"));
    assert_eq!(widget.context().view, FileView::Grid);
    assert!(app.choose("view-list"));
    assert_eq!(widget.context().view, FileView::List);
    assert!(app.choose("view-trash"));
    assert!(widget.context().in_trash);
    assert!(app.choose("file-new-folder"), "leaves the trash for it");
    assert!(!widget.context().in_trash);
    assert!(widget.with_state(|s| matches!(s.ask(), Ask::Input(_))));
    widget.with_state(|s| s.cancel_input());
    assert!(app.choose("file-upload"));
    widget.with_state(|s| s.cancel_input());
    assert!(app.choose("view-transfers"));
    assert_eq!(widget.with_state(|s| s.side()), Side::Transfers);
    std::fs::write(w.root().join("new.txt"), b"x").unwrap();
    assert!(app.choose("file-rescan"));
    assert_eq!(widget.with_state(|s| s.rows().len()), 1);
    assert!(!app.choose("file-delete"), "no other entry");
}
