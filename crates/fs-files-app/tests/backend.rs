//! The `Backend` of `File` and `Folder` over the socket: `FilesRemote` against a real server, repository and folder.
//!
//! Pattern: Integration test — the server runs on its own thread and socket; the backends are the ones the window uses.

mod support;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{FileBackend, FolderBackend};
use fs_types::BasicRules;
use support::{names, url, v, world};

#[test]
fn the_server_describes_all_five_types() {
    let w = world();
    let mut got: Vec<String> = w
        .remote
        .describe()
        .unwrap()
        .into_iter()
        .map(|d| d.name)
        .collect();
    got.sort();
    assert_eq!(
        got,
        ["Album", "File", "Folder", "Picture", "StorageBackend"]
    );
}

#[test]
fn files_are_uploaded_listed_changed_trashed_restored_and_downloaded() {
    let w = world();
    let (files, folders) = (
        FileBackend(w.remote.clone()),
        FolderBackend(w.remote.clone()),
    );
    let local = w.dir.join("note.txt");
    std::fs::write(&local, "hello").unwrap();

    let docs = folders.run("create", &v(&[("name", "Docs")])).unwrap();
    let docs_id = docs.id.unwrap();
    assert_eq!(names(&folders, &Values::new()), ["Docs"]);

    let up = files
        .run(
            "upload",
            &v(&[("source", &url(&local)), ("folder", &docs_id)]),
        )
        .unwrap();
    let id = up.id.unwrap();
    assert_eq!(names(&files, &Values::new()), ["note.txt"]);
    let row = &files.list(&Values::new()).unwrap()[0];
    assert_eq!(row.values["size"], ["5"]);
    assert_eq!(row.values["folder"], std::slice::from_ref(&docs_id));
    assert_eq!(row.values["state"], ["live"]);

    // the same name again is refused as a finding on the field `name`, and keep-both gives it another name
    let again = v(&[("source", &url(&local)), ("folder", &docs_id)]);
    assert!(
        matches!(files.run("upload", &again), Err(BackendError::Rejected(f)) if f[0].field == "name")
    );
    let mut both = again.clone();
    both.insert("onConflict".into(), vec!["keep-both".into()]);
    files.run("upload", &both).unwrap();
    assert_eq!(names(&files, &Values::new()), ["note (2).txt", "note.txt"]);

    files
        .run("rename", &v(&[("file", &id), ("name", "renamed.txt")]))
        .unwrap();
    assert_eq!(names(&files, &v(&[("name", "renamed")])), ["renamed.txt"]);
    let mut top = v(&[("file", &id)]);
    top.insert("folder".into(), Vec::new());
    files.run("move", &top).unwrap();
    assert!(
        files
            .list(&Values::new())
            .unwrap()
            .iter()
            .any(|r| r.id == id && !r.values.contains_key("folder"))
    );
    let copy = files
        .run("copy", &v(&[("file", &id), ("onConflict", "keep-both")]))
        .unwrap();
    assert_eq!(
        names(&files, &v(&[("name", "renamed (copy)")])),
        ["renamed (copy).txt"]
    );
    assert!(copy.values["copy"][0] != id);
    files
        .run(
            "classify",
            &v(&[("file", &id), ("sensitive", "dpv:Health")]),
        )
        .unwrap();
    let marked = files.list(&v(&[("name", "renamed.txt")])).unwrap();
    assert_eq!(marked[0].values["sensitive"], ["dpv:Health"]);

    let target = w.dir.join("out.txt");
    files
        .run("download", &v(&[("file", &id), ("target", &url(&target))]))
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"hello");
    assert!(
        files
            .run("download", &v(&[("file", &id), ("target", &url(&target))]))
            .is_err()
    );

    files.run("delete", &v(&[("file", &id)])).unwrap();
    assert!(names(&files, &v(&[("name", "renamed.txt")])).is_empty());
    assert_eq!(names(&files, &v(&[("state", "trashed")])), ["renamed.txt"]);
    files.run("restore", &v(&[("file", &id)])).unwrap();
    assert_eq!(
        names(&files, &v(&[("name", "renamed.txt")])),
        ["renamed.txt"]
    );
    files.run("delete", &v(&[("file", &id)])).unwrap();
    files.run("purge", &v(&[("file", &id)])).unwrap();
    assert!(names(&files, &v(&[("state", "trashed")])).is_empty());
    let empty = files.run("emptyTrash", &Values::new()).unwrap();
    assert_eq!(empty.values["purged"], ["0"]);
    assert!(matches!(
        files.run("resolve", &v(&[("file", &id)])),
        Err(BackendError::NotFound)
    ));
}

#[test]
fn folders_are_created_renamed_moved_deleted_and_rescanned() {
    let w = world();
    let folders = FolderBackend(w.remote.clone());
    let a = folders
        .run("create", &v(&[("name", "A")]))
        .unwrap()
        .id
        .unwrap();
    let b = folders
        .run("create", &v(&[("name", "B"), ("parent", &a)]))
        .unwrap()
        .id
        .unwrap();
    assert_eq!(names(&folders, &Values::new()), ["A", "B"]);
    assert_eq!(names(&folders, &v(&[("name", "b")])), ["B"]);
    assert!(matches!(
        folders.run("create", &v(&[("name", "A")])),
        Err(BackendError::Rejected(f)) if f[0].field == "name"
    ));
    assert!(matches!(
        folders.run("create", &v(&[("name", "a/b")])),
        Err(BackendError::Rejected(_))
    ));
    folders
        .run("rename", &v(&[("folder", &b), ("name", "C")]))
        .unwrap();
    let mut top = v(&[("folder", &b)]);
    top.insert("parent".into(), Vec::new());
    folders.run("move", &top).unwrap();
    let rows = folders.list(&Values::new()).unwrap();
    let moved = rows.iter().find(|r| r.id == b).unwrap();
    assert!(!moved.values.contains_key("parent"));
    folders.run("resolve", &v(&[("folder", &b)])).unwrap();

    std::fs::write(w.dir.join("files").join("A").join("found.txt"), "x").unwrap();
    let scan = folders.run("rescan", &v(&[("folder", &a)])).unwrap();
    assert_eq!(scan.values["added"], ["found.txt"]);
    folders.run("delete", &v(&[("folder", &b)])).unwrap();
    assert_eq!(names(&folders, &Values::new()), ["A"]);
}

#[test]
fn ids_and_references_are_absolute_iris_that_the_basic_type_accepts() {
    let w = world();
    let (files, folders) = (
        FileBackend(w.remote.clone()),
        FolderBackend(w.remote.clone()),
    );
    let dir = folders
        .run("create", &v(&[("name", "D")]))
        .unwrap()
        .id
        .unwrap();
    assert!(dir.starts_with("urn:uuid:"), "{dir}");
    let local = w.dir.join("a b.txt");
    std::fs::write(&local, "x").unwrap();
    // a path with a space typed as a link: the percent code is decoded
    let encoded = format!("file://{}", local.display()).replace(' ', "%20");
    let id = files
        .run("upload", &v(&[("source", &encoded), ("folder", &dir)]))
        .unwrap()
        .id
        .unwrap();
    let described = w.remote.describe().unwrap();
    let file = described.iter().find(|d| d.name == "File").unwrap();
    let rules = BasicRules::standard();
    let row = &files.list(&Values::new()).unwrap()[0];
    assert_eq!(row.id, id);
    assert_eq!(row.values["name"], ["a b.txt"]);
    for (field, values) in &row.values {
        for value in values {
            file.check_value(&rules, field, value)
                .unwrap_or_else(|e| panic!("{field}={value}: {e:?}"));
        }
    }
    // restoreTo: trash it, restore into the top level
    files.run("delete", &v(&[("file", &id)])).unwrap();
    files.run("restoreTo", &v(&[("file", &id)])).unwrap();
    assert!(
        files.list(&Values::new()).unwrap()[0]
            .values
            .get("folder")
            .is_none()
    );
}

#[test]
fn a_missing_field_leaves_a_change_unchanged_and_an_empty_one_clears() {
    let w = world();
    let (files, folders) = (
        FileBackend(w.remote.clone()),
        FolderBackend(w.remote.clone()),
    );
    let dir = folders
        .run("create", &v(&[("name", "D")]))
        .unwrap()
        .id
        .unwrap();
    let local = w.dir.join("n.txt");
    std::fs::write(&local, "x").unwrap();
    let id = files
        .run("upload", &v(&[("source", &url(&local)), ("folder", &dir)]))
        .unwrap()
        .id
        .unwrap();
    files
        .run(
            "classify",
            &v(&[("file", &id), ("sensitive", "dpv:Health")]),
        )
        .unwrap();
    // move without the field: unchanged; classify without the field: the marking stays
    files.run("move", &v(&[("file", &id)])).unwrap();
    files.run("classify", &v(&[("file", &id)])).unwrap();
    let row = &files.list(&Values::new()).unwrap()[0];
    assert_eq!(row.values["folder"], [dir.clone()]);
    assert_eq!(row.values["sensitive"], ["dpv:Health"]);
    // an empty list clears
    let mut cleared = v(&[("file", &id)]);
    cleared.insert("sensitive".into(), Vec::new());
    files.run("classify", &cleared).unwrap();
    assert!(
        !files.list(&Values::new()).unwrap()[0]
            .values
            .contains_key("sensitive")
    );
    let mut top = v(&[("file", &id)]);
    top.insert("folder".into(), Vec::new());
    files.run("move", &top).unwrap();
    assert!(
        !files.list(&Values::new()).unwrap()[0]
            .values
            .contains_key("folder")
    );
    // folders: the same rule for `parent`
    let child = folders
        .run("create", &v(&[("name", "C"), ("parent", &dir)]))
        .unwrap()
        .id
        .unwrap();
    folders.run("move", &v(&[("folder", &child)])).unwrap();
    let rows = folders.list(&Values::new()).unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == child).unwrap().values["parent"],
        [dir]
    );
}

#[test]
fn a_find_with_an_input_of_a_command_the_core_lacks_is_refused() {
    let w = world();
    let files = FileBackend(w.remote.clone());
    assert!(matches!(
        files.list(&v(&[("text", "x")])),
        Err(BackendError::Unavailable(t)) if t == "unsupported:text"
    ));
    assert!(matches!(
        files.list(&v(&[("picture", "x")])),
        Err(BackendError::Unavailable(_))
    ));
}
