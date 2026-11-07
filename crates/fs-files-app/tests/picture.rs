//! The whole program as a picture, through the shell's real path. No window is opened.
//!
//! Pattern: Pixel tests — skipped with a message where there is no graphics adapter. Pictures are kept
//! where `FS_COMP_SHOTS` says. `draw_to_png` holds the process-wide lock, so everything here takes turns.
//! A hang here is a deadlock.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use common::World;
use fs_components_app::{AppError, AppWidget};
use fs_components_workbenches::file_manager::{CarryKind, FileView, PreviewContent, SortKey};
use fs_files_app::screen_widget;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn out(name: &str) -> PathBuf {
    let dir = std::env::var_os("FS_COMP_SHOTS").map_or_else(
        || std::env::temp_dir().join(format!("fs-files-app-{}", std::process::id())),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn read_and_clear(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    if std::env::var_os("FS_COMP_SHOTS").is_none() {
        std::fs::remove_file(path).unwrap();
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
    bytes
}

fn draw(app: &AppWidget, name: &str, size: (u32, u32)) -> Option<Vec<u8>> {
    let path = out(name);
    match app.picture(size.0, size.1, &path) {
        Err(AppError::Picture(fs_components_render::RenderError::NoGraphics(why))) => {
            eprintln!("skipped: {why}");
            None
        }
        other => {
            other.unwrap();
            Some(read_and_clear(&path))
        }
    }
}

fn world() -> World {
    let w = World::encrypted();
    let projects = w.folder(None, "Projects");
    let photos = w.folder(None, "Holiday photos 2026");
    w.folder(Some(projects.id), "FreeSynergy");
    w.folder(Some(projects.id), "Garden");
    w.file(
        None,
        "notes with spaces.txt",
        b"First line of the notes.\nSecond line.\n\nAnd so on.\n",
    );
    w.file(
        None,
        "Übergrößenträger 日本語.md",
        "# Title\n\nÜber Größen.\n".as_bytes(),
    );
    let long = format!(
        "{}.txt",
        "a very long name that goes on and on ".repeat(5).trim()
    );
    w.file(None, &long, b"long");
    w.file(None, "data.bin", &[0u8, 1, 2, 3, 255, 254]);
    let marked = w.file(None, "blood test results.pdf", b"%PDF-1.7 private");
    w.mark(marked.id);
    w.file(Some(photos.id), "beach.txt", b"sand");
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_fn(96, 64, |x, y| {
        image::Rgba([(x * 2) as u8, (y * 3) as u8, 160, 255])
    })
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    w.file(None, "gradient.png", png.get_ref());
    w
}

fn select_named(widget: &fs_components_workbenches::file_manager::FileManagerWidget, name: &str) {
    widget.with_state(|s| {
        let key = s
            .rows()
            .iter()
            .find(|r| r.name.contains(name))
            .unwrap()
            .key
            .clone();
        s.select(&key);
    });
}

#[test]
fn the_program_draws_through_picture_and_the_views_differ() {
    let _one = serial();
    let w = world();
    let widget = w.widget();
    let app = screen_widget(widget.clone()).unwrap();
    let Some(list) = draw(&app, "files-list.png", (1300, 800)) else {
        return;
    };
    widget.with_state(|s| {
        s.open_folder(None);
        let key = s
            .rows()
            .iter()
            .find(|r| r.name == "Projects")
            .unwrap()
            .key
            .clone();
        s.open_folder(Some(&key));
        s.open_folder(None);
    });
    select_named(&widget, "notes with spaces");
    let selected = draw(&app, "files-list-selected.png", (1300, 800)).unwrap();
    assert_ne!(list, selected);
    widget.with_state(|s| s.set_view(FileView::Grid));
    let grid = draw(&app, "files-grid.png", (1300, 800)).unwrap();
    assert_ne!(selected, grid);
    widget.with_state(|s| s.set_view(FileView::List));
    select_named(&widget, "blood test");
    let marked = draw(&app, "files-marked.png", (1300, 800)).unwrap();
    assert_ne!(selected, marked);
    select_named(&widget, "gradient");
    assert!(widget.with_state(|s| matches!(s.content(), Some(PreviewContent::Picture(_)))));
    assert!(draw(&app, "files-picture.png", (1300, 800)).is_some());
    widget.with_state(|s| s.set_sort(SortKey::Size));
    assert!(draw(&app, "files-sorted.png", (1300, 800)).is_some());
}

#[test]
fn the_question_on_a_taken_name_and_the_trash_are_drawn() {
    let _one = serial();
    let w = world();
    let widget = w.widget();
    let app = screen_widget(widget.clone()).unwrap();
    select_named(&widget, "data.bin");
    widget.with_state(|s| {
        s.begin_rename();
        s.set_input("notes with spaces.txt");
        s.submit_input();
    });
    let Some(question) = draw(&app, "files-conflict.png", (1300, 800)) else {
        return;
    };
    assert!(!question.is_empty());
    widget.with_state(|s| s.name_taken_back());
    assert!(draw(&app, "files-rename.png", (1300, 800)).is_some());
    widget.with_state(|s| {
        s.cancel_input();
        s.discard_input();
        s.ask_delete();
        s.really();
    });
    select_named(&widget, "blood test");
    widget.with_state(|s| {
        s.ask_delete();
        s.really();
        s.open_trash();
    });
    assert!(draw(&app, "files-trash.png", (1300, 800)).is_some());
    widget.with_state(|s| s.ask_empty_trash());
    assert!(draw(&app, "files-empty-trash.png", (1300, 800)).is_some());
    assert!(draw(&app, "files-empty-trash-narrow.png", (420, 800)).is_some());
    widget.with_state(|s| {
        s.keep();
        s.open_folder(None);
    });
    select_named(&widget, "notes with spaces");
    widget.with_state(|s| s.carry(CarryKind::Move));
    assert!(draw(&app, "files-carrying.png", (1300, 800)).is_some());
    assert!(draw(&app, "files-narrow-detail.png", (420, 800)).is_some());
    widget.with_state(|s| {
        s.stop_carrying();
        s.close_detail();
    });
    assert!(draw(&app, "files-narrow.png", (420, 800)).is_some());
    widget.with_state(|s| s.open_transfers());
    assert!(draw(&app, "files-transfers.png", (1300, 800)).is_some());
    std::fs::write(w.root().join("from outside.txt"), b"x").unwrap();
    widget.with_state(|s| s.rescan());
    assert!(draw(&app, "files-report.png", (1300, 800)).is_some());
    widget.with_state(|s| s.open_filter());
    assert!(draw(&app, "files-filter.png", (1300, 800)).is_some());
}
