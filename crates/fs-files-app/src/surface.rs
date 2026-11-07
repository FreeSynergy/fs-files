//! The program as a screen of the shell.
//!
//! Pattern: Adapter — the workbench as a `Surface` of the application shell; the only screen, so the
//! shell draws no navigation. Command — a menu entry is an id, and one handler turns it into a change of
//! the workbench's state. Only what works now is offered, and nothing destructive: those are two acts
//! in the workbench.

use std::sync::Arc;

use fs_components_app::{AppError, AppWidget, Element, Menu, MenuEntry, Screen, Surface};
use fs_components_workbenches::file_manager::{FileManagerWidget, FileSource, FileView};

use crate::words::{TITLE, labels, menu};

const NEW_FOLDER: &str = "file-new-folder";
const UPLOAD: &str = "file-upload";
const RESCAN: &str = "file-rescan";
const LIST: &str = "view-list";
const GRID: &str = "view-grid";
const TRASH: &str = "view-trash";
const TRANSFERS: &str = "view-transfers";

/// The file manager as a surface.
pub struct FilesScreen(pub FileManagerWidget);

impl Surface for FilesScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }
}

/// The shell around `widget`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn screen_widget(widget: FileManagerWidget) -> Result<AppWidget, AppError> {
    let screen = Screen::new("files", TITLE, Arc::new(FilesScreen(widget.clone())));
    let app = AppWidget::new(TITLE, vec![screen])?;
    app.set_menus(menus_of(&widget), move |id| run(&widget, id))?;
    Ok(app)
}

/// What `widget` can do from a menu: what the source cannot do is not offered.
fn menus_of(widget: &FileManagerWidget) -> Vec<Menu> {
    let caps = widget.with_state(|s| s.capabilities());
    let mut file = Vec::new();
    if caps.change {
        file.push(MenuEntry::new(NEW_FOLDER, menu::NEW_FOLDER));
    }
    if caps.upload {
        file.push(MenuEntry::new(UPLOAD, menu::UPLOAD));
    }
    if caps.rescan {
        file.push(MenuEntry::new(RESCAN, menu::RESCAN));
    }
    let mut view = vec![
        MenuEntry::new(LIST, menu::LIST),
        MenuEntry::new(GRID, menu::GRID),
    ];
    if caps.trash {
        view.push(MenuEntry::new(TRASH, menu::TRASH));
    }
    if caps.upload || caps.download {
        view.push(MenuEntry::new(TRANSFERS, menu::TRANSFERS));
    }
    let mut menus = Vec::new();
    if !file.is_empty() {
        menus.push(Menu::new(menu::FILE, file));
    }
    menus.push(Menu::new(menu::VIEW, view));
    menus
}

/// One menu entry: runs on the interface thread with no lock of the shell held. What only works outside
/// the trash goes there first.
fn run(widget: &FileManagerWidget, id: &str) {
    widget.with_state(|s| {
        let leave_trash = matches!(id, NEW_FOLDER | UPLOAD | RESCAN) && s.in_trash();
        if leave_trash {
            s.open_folder(None);
        }
        match id {
            NEW_FOLDER => s.begin_new_folder(),
            UPLOAD => s.begin_upload(),
            RESCAN => s.rescan(),
            LIST => s.set_view(FileView::List),
            GRID => s.set_view(FileView::Grid),
            TRASH => s.open_trash(),
            TRANSFERS => s.open_transfers(),
            _ => {}
        }
    });
    widget.refresh();
}

/// The shell around the workbench over `source`.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(source: Arc<dyn FileSource>) -> Result<AppWidget, AppError> {
    screen_widget(FileManagerWidget::new(source, labels()))
}
