//! The View menu of the file window: the faces it offers, the choice that is kept, a kept choice that is damaged.
//!
//! Pattern: Integration test — the window of the program over a real server; the menu is chosen as a press would.

mod support;

use fs_components_workbenches::file_manager::FileView;
use fs_files_app::settings;
use fs_files_app::window::files_widget;
use support::world;

fn entries(app: &fs_components_app::AppWidget) -> Vec<(String, String, bool)> {
    app.with_shell(|s| {
        s.menus()
            .iter()
            .flat_map(|m| m.entries.iter().map(|e| (e.id.clone(), e.label.clone(), e.disabled)))
            .collect()
    })
}

#[test]
fn the_menus_name_the_faces_and_a_chosen_one_is_kept() {
    let w = world();
    let path = w.dir.join("kept").join(settings::FILE);
    let app = files_widget(&w.remote, FileView::List, Some(path.clone())).unwrap();
    assert_eq!(
        entries(&app),
        [
            ("program-quit".into(), "Quit".into(), false),
            ("face-list".into(), "Face: List (current)".into(), true),
            ("face-grid".into(), "Face: Grid".into(), false),
        ]
    );
    assert!(!path.exists(), "nothing is kept before a choice");

    assert!(app.choose("face-grid"));
    assert_eq!(settings::load(&path), FileView::Grid);
    let now = entries(&app);
    assert!(!now[1].2 && now[2].2, "the shown face is the one that cannot be chosen: {now:?}");
    assert_eq!(now[2].1, "Face: Grid (current)");

    // the next start: what was kept shows, then the person goes back
    let again = files_widget(&w.remote, settings::load(&path), Some(path.clone())).unwrap();
    assert_eq!(entries(&again)[2].1, "Face: Grid (current)");
    assert!(again.choose("face-list"));
    assert_eq!(settings::load(&path), FileView::List);
    assert!(!again.choose("face-nope"), "an unknown entry does nothing");
}

#[test]
fn a_damaged_kept_choice_gives_the_standard_and_the_next_choice_repairs_it() {
    let w = world();
    let path = w.dir.join(settings::FILE);
    std::fs::write(&path, "view=\u{0}\u{0}tiles").unwrap();
    let view = settings::load(&path);
    assert_eq!(view, FileView::List);
    let app = files_widget(&w.remote, view, Some(path.clone())).unwrap();
    assert!(entries(&app)[1].2, "the list is the shown face");
    assert!(app.choose("face-grid"));
    assert_eq!(settings::load(&path), FileView::Grid);
}
