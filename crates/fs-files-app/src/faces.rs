//! The menus of the file window: Program (Quit) and View (the faces of the file window), the choice of a face is kept.
//!
//! Pattern: Command table — a menu entry is named by a stable id and one handler maps the id to what happens. There is
//! one face of the file window today; it has two ways to show a folder (list, grid), and these are what the menu offers.
//! No look and no render code here (ADR 0066): only data, wiring and the keeping of the choice.

use std::path::PathBuf;
use std::sync::Arc;

use fs_components_app::{AppError, AppWidget, Menu, MenuEntry};
use fs_components_fields::Say;
use fs_components_workbenches::file_manager::{FileManagerWidget, FileView};

use crate::settings;

const QUIT: &str = "program-quit";
const LIST: &str = "face-list";
const GRID: &str = "face-grid";

/// The view a menu entry id names; any other id is `None`.
fn view_of(id: &str) -> Option<FileView> {
    match id {
        LIST => Some(FileView::List),
        GRID => Some(FileView::Grid),
        _ => None,
    }
}

/// The menus; the entry of the face that is shown cannot be chosen again and says so.
pub(crate) fn menus(say: &dyn Say, current: FileView) -> Vec<Menu> {
    let face = |id: &str, view: FileView, key: &str| {
        let mut label = format!("{}: {}", say.say("menu.face"), say.say(key));
        if view == current {
            label = format!("{label} ({})", say.say("face.current"));
        }
        let entry = MenuEntry::new(id, label);
        if view == current {
            entry.disabled()
        } else {
            entry
        }
    };
    vec![
        Menu::new(
            say.say("menu.program"),
            vec![MenuEntry::new(QUIT, say.say("menu.quit"))],
        ),
        Menu::new(
            say.say("menu.view"),
            vec![
                face(LIST, FileView::List, "face.list"),
                face(GRID, FileView::Grid, "face.grid"),
            ],
        ),
    ]
}

/// Put the menus into the window. A chosen face is shown at once and kept in `settings` (if there is a place);
/// a place that cannot be written is told on stderr, the face still changes for this run.
///
/// # Errors
/// [`AppError`] if the shell refuses the menus.
pub(crate) fn install(
    app: &AppWidget,
    widget: &FileManagerWidget,
    say: &Arc<dyn Say>,
    view: FileView,
    settings: Option<PathBuf>,
) -> Result<(), AppError> {
    let (app2, widget2, say2) = (app.clone(), widget.clone(), Arc::clone(say));
    app.set_menus(menus(&**say, view), move |id| {
        if id == QUIT {
            app2.quit();
            return;
        }
        let Some(chosen) = view_of(id) else { return };
        widget2.with_state(|s| s.set_view(chosen));
        widget2.refresh();
        if let Some(path) = &settings
            && let Err(e) = settings::save(path, chosen)
        {
            eprintln!("fs-files: {e}");
        }
        if let Err(e) = install(&app2, &widget2, &say2, chosen, settings.clone()) {
            eprintln!("fs-files: {e}");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::words::English;

    #[test]
    fn the_menus_offer_the_two_ways_and_the_one_shown_cannot_be_chosen() {
        let m = menus(&English, FileView::List);
        assert_eq!(m.len(), 2);
        let ids: Vec<_> = m[1]
            .entries
            .iter()
            .map(|e| (e.id.as_str(), e.disabled))
            .collect();
        assert_eq!(ids, [(LIST, true), (GRID, false)]);
        assert_eq!(m[1].entries[0].label, "Face: List (current)");
        assert_eq!(m[1].entries[1].label, "Face: Grid");
        let m = menus(&English, FileView::Grid);
        assert!(!m[1].entries[0].disabled && m[1].entries[1].disabled);
        assert_eq!(view_of(GRID), Some(FileView::Grid));
        assert_eq!(view_of("nope"), None);
    }
}
