//! What the backends do with a field that is missing or empty (the rule for changing), over the socket.
//!
//! Pattern: Integration test — the server runs on its own thread and socket; the backends are the ones the window uses.

mod support;

use fs_admin_frontend::{Backend, Values};
use fs_files_app::{FileBackend, FolderBackend};
use support::{url, v, world};

#[test]
fn a_missing_field_leaves_a_change_unchanged_and_an_empty_one_clears() {
    let w = world();
    let (files, folders) = (
        FileBackend(w.remote.clone()),
        FolderBackend(w.remote.clone()),
    );
    let dir = folders
        .run("create", None, &v(&[("name", "D")]))
        .unwrap()
        .id
        .unwrap();
    let local = w.dir.join("n.txt");
    std::fs::write(&local, "x").unwrap();
    let id = files
        .run(
            "upload",
            None,
            &v(&[("source", &url(&local)), ("folder", &dir)]),
        )
        .unwrap()
        .id
        .unwrap();
    files
        .run(
            "classify",
            None,
            &v(&[("file", &id), ("sensitive", "dpv:Health")]),
        )
        .unwrap();
    // move without the field: unchanged; classify without the field: the marking stays
    files.run("move", None, &v(&[("file", &id)])).unwrap();
    files.run("classify", None, &v(&[("file", &id)])).unwrap();
    let row = &files.list(&Values::new()).unwrap()[0];
    assert_eq!(row.values["folder"], std::slice::from_ref(&dir));
    assert_eq!(row.values["sensitive"], ["dpv:Health"]);
    // an empty list clears
    let mut cleared = v(&[("file", &id)]);
    cleared.insert("sensitive".into(), Vec::new());
    files.run("classify", None, &cleared).unwrap();
    assert!(
        !files.list(&Values::new()).unwrap()[0]
            .values
            .contains_key("sensitive")
    );
    let mut top = v(&[("file", &id)]);
    top.insert("folder".into(), Vec::new());
    files.run("move", None, &top).unwrap();
    assert!(
        !files.list(&Values::new()).unwrap()[0]
            .values
            .contains_key("folder")
    );
    // folders: the same rule for `parent`
    let child = folders
        .run("create", None, &v(&[("name", "C"), ("parent", &dir)]))
        .unwrap()
        .id
        .unwrap();
    folders
        .run("move", None, &v(&[("folder", &child)]))
        .unwrap();
    let rows = folders.list(&Values::new()).unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == child).unwrap().values["parent"],
        [dir]
    );
}
