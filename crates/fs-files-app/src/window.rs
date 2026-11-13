//! The window of `fs-files` (ADR 0063, 0065, 0066): the standard shell around the generated Admin frontend.
//!
//! Pattern: Facade — one call turns the descriptions the server reports and the `Backend` of each type into the
//! window. No look and no render code here; the program only gives data and wiring (ADR 0066, rule of 2026-11-10).

use std::path::PathBuf;
use std::sync::Arc;

use fs_admin_frontend::view::{AboutInfo, AdminSurface};
use fs_admin_frontend::window::{TypeView, admin_window};
use fs_admin_frontend::{Backend, Session, SystemClock};
use fs_components_app::{AppError, AppWidget, Element, Screen, Surface};
use fs_components_fields::{Look, Say};
use fs_components_styles::StyleId;
use fs_components_workbenches::file_manager::{FileLabels, FileManagerWidget, FileView};
use fs_types::TypeDescription;

use crate::backend_file::FileBackend;
use crate::backend_folder::FolderBackend;
use crate::backend_picture::{AlbumBackend, PictureBackend};
use crate::backend_storage::StorageBackendBackend;
use crate::words::English;
use crate::{FilesRemote, RemoteFileSource};

/// The title of the window.
pub const TITLE: &str = "FreeSynergy Files";

/// Where the help lives until the wiki answers (ADR 0065).
pub const HELP_URL: &str = "https://github.com/FreeSynergy/fs-files";

/// The `Backend` for the data type `name`; a type this version does not know has none.
fn backend_of(name: &str, remote: &Arc<FilesRemote>) -> Option<Arc<dyn Backend>> {
    let remote = Arc::clone(remote);
    Some(match name {
        "File" => Arc::new(FileBackend(remote)),
        "Folder" => Arc::new(FolderBackend(remote)),
        "StorageBackend" => Arc::new(StorageBackendBackend(remote)),
        "Picture" => Arc::new(PictureBackend(remote)),
        "Album" => Arc::new(AlbumBackend),
        _ => return None,
    })
}

/// The About page: the version is the one of the package.
fn about() -> AboutInfo {
    AboutInfo {
        name: "FreeSynergy.Files".into(),
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        origin: "source".into(),
        licence: env!("CARGO_PKG_LICENSE").into(),
        repository: "https://github.com/FreeSynergy/fs-files".into(),
    }
}

/// The window over `remote`, a screen per data type the server describes.
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn app_widget(
    remote: &Arc<FilesRemote>,
    descriptions: Vec<TypeDescription>,
) -> Result<AppWidget, AppError> {
    let say: Arc<dyn Say> = Arc::new(English);
    let mut descriptions = descriptions;
    // The order of the navigation: what people use first; a type this list does not know goes last.
    let rank = |n: &str| {
        ["File", "Folder", "Picture", "Album", "StorageBackend"]
            .iter()
            .position(|k| *k == n)
    };
    descriptions.sort_by_key(|d| rank(&d.name).unwrap_or(usize::MAX));
    let views = descriptions
        .into_iter()
        .filter_map(|d| {
            let backend = backend_of(&d.name, remote)?;
            let (id, label) = (d.name.clone(), d.label.clone());
            let session = Session::new(d, backend, Arc::new(SystemClock::new()));
            let surface = AdminSurface::new(
                session,
                Look::new(StyleId::Admin, false),
                Arc::clone(&say),
                None,
            );
            Some(TypeView {
                id,
                label,
                surface: Arc::new(surface),
            })
        })
        .collect();
    admin_window(TITLE, &say, views, about(), HELP_URL.into())
}

/// The file workbench (ADR 0066) as the screen of the shell.
struct FilesScreen(FileManagerWidget);

impl Surface for FilesScreen {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        Box::new(self.0.build(width, height))
    }

    fn styles(&self) -> String {
        self.0.styles()
    }

    /// The file window has a status bar of its own (how many things, what is selected): no empty strip under it.
    fn status_line(&self) -> bool {
        false
    }
}

/// The standard window: the file workbench over the server in the standard shell. The generated Admin frontend
/// ([`app_widget`]) stays behind `--facet admin`. It starts with `view`; the face the person chooses in the View menu is
/// shown at once and kept in the file `settings` (none: not kept).
///
/// # Errors
/// [`AppError`] if the shell cannot be made.
pub fn files_widget(
    remote: &Arc<FilesRemote>,
    view: FileView,
    settings: Option<PathBuf>,
) -> Result<AppWidget, AppError> {
    let source = Arc::new(RemoteFileSource(Arc::clone(remote)));
    let widget = FileManagerWidget::new(source, FileLabels::default());
    widget.with_state(|s| s.set_view(view));
    widget.pin_by_names(crate::xdg::user_folder_names());
    let screen = Screen::new("files", TITLE, Arc::new(FilesScreen(widget.clone())));
    let app = AppWidget::new(TITLE, vec![screen])?;
    let say: Arc<dyn Say> = Arc::new(English);
    crate::faces::install(&app, &widget, &say, view, settings)?;
    Ok(app)
}
