//! The `Backend` of each data type over the socket: `FilesRemote` against a real server, repository and folder.
//!
//! Pattern: Integration test — the server runs on its own thread and socket; the backends are the ones the window uses.

use std::path::Path;
use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Values};
use fs_files_app::{
    AlbumBackend, FileBackend, FilesRemote, FolderBackend, PictureBackend, StorageBackendBackend,
};
use fs_files_core::{FileRepo, LocalFolder};
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{bind_socket, serve};
use fs_objects::{Store, TursoStore};

const ME: &str = "https://local.invalid/users/owner";

fn start_server(socket: &Path, root: &Path) {
    let (socket, root) = (socket.to_path_buf(), root.to_path_buf());
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let backend = Arc::new(LocalFolder::open(&root).await.unwrap());
            let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
            let server = FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo);
            let bound = bind_socket(&socket).await.unwrap();
            up.send(()).unwrap();
            serve(bound, server.into_client().client, std::future::pending())
                .await
                .ok();
        });
    });
    started.recv().unwrap();
}

fn v(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(k, x)| ((*k).to_owned(), vec![(*x).to_owned()]))
        .collect()
}

fn url(p: &Path) -> String {
    format!("file://{}", p.display())
}

struct World {
    _dir: tempfile::TempDir,
    dir: std::path::PathBuf,
    remote: Arc<FilesRemote>,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let (socket, root) = (dir.path().join("files.sock"), dir.path().join("files"));
    start_server(&socket, &root);
    let remote = Arc::new(FilesRemote::connect(&socket).unwrap());
    World {
        dir: dir.path().to_path_buf(),
        _dir: dir,
        remote,
    }
}

fn names(b: &dyn Backend, query: &Values) -> Vec<String> {
    let mut n: Vec<String> = b
        .list(query)
        .unwrap()
        .iter()
        .map(|r| r.values["name"][0].clone())
        .collect();
    n.sort();
    n
}

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
    files.run("move", &v(&[("file", &id)])).unwrap();
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
    folders.run("move", &v(&[("folder", &b)])).unwrap();
    let rows = folders.list(&Values::new()).unwrap();
    let moved = rows.iter().find(|r| r.id == b).unwrap();
    assert!(!moved.values.contains_key("parent"));
    folders.run("resolve", &v(&[("folder", &b)])).unwrap();

    std::fs::write(w.dir.join("files").join("found.txt"), "x").unwrap();
    let scan = folders.run("rescan", &v(&[("folder", &a)])).unwrap();
    assert!(scan.values.contains_key("skipped"));
    folders.run("delete", &v(&[("folder", &b)])).unwrap();
    assert_eq!(names(&folders, &Values::new()), ["A"]);
}

#[test]
fn pictures_are_the_image_files_and_albums_do_not_exist_yet() {
    let w = world();
    let (files, pictures) = (
        FileBackend(w.remote.clone()),
        PictureBackend(w.remote.clone()),
    );
    let (photo, text) = (w.dir.join("p.png"), w.dir.join("t.txt"));
    std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap();
    std::fs::write(&text, "x").unwrap();
    let id = files
        .run(
            "upload",
            &v(&[("source", &url(&photo)), ("mediaType", "image/png")]),
        )
        .unwrap()
        .id
        .unwrap();
    files.run("upload", &v(&[("source", &url(&text))])).unwrap();
    let rows = pictures.list(&Values::new()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].values["file"], std::slice::from_ref(&id));

    pictures
        .run(
            "sensitiveMark",
            &v(&[("picture", &id), ("sensitive", "dpv:Health")]),
        )
        .unwrap();
    assert_eq!(
        pictures.list(&Values::new()).unwrap()[0].values["sensitive"],
        ["dpv:Health"]
    );
    let target = w.dir.join("copy.png");
    pictures
        .run(
            "download",
            &v(&[("picture", &id), ("target", &url(&target))]),
        )
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap().len(), 4);

    assert!(AlbumBackend.list(&Values::new()).unwrap().is_empty());
}

#[test]
fn what_the_core_lacks_is_refused_with_the_name_and_never_faked() {
    let w = world();
    let unsupported = |e: Result<_, BackendError>, name: &str| match e {
        Err(BackendError::Unavailable(t)) => assert_eq!(t, format!("unsupported:{name}")),
        other => panic!("{name}: {other:?}"),
    };
    unsupported(
        FileBackend(w.remote.clone()).run("preview", &Values::new()),
        "preview",
    );
    unsupported(
        FileBackend(w.remote.clone()).run("shareCreate", &Values::new()),
        "shareCreate",
    );
    unsupported(
        FolderBackend(w.remote.clone()).run("archive", &Values::new()),
        "archive",
    );
    unsupported(
        PictureBackend(w.remote.clone()).run("tagAdd", &Values::new()),
        "tagAdd",
    );
    unsupported(
        StorageBackendBackend(w.remote.clone()).run("quota", &Values::new()),
        "quota",
    );
    unsupported(AlbumBackend.run("create", &Values::new()), "create");
}

#[test]
fn storage_backends_are_listed_and_a_bad_configuration_is_refused() {
    let w = world();
    let storage = StorageBackendBackend(w.remote.clone());
    let rows = storage.list(&Values::new()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "main");
    assert_eq!(rows[0].values["state"], ["active"]);
    assert!(
        storage
            .run("create", &v(&[("config", "{\"kind\":\"nope\"}")]))
            .is_err()
    );
    let second = serde_json_config("second");
    storage.run("create", &v(&[("config", &second)])).unwrap();
    assert_eq!(storage.list(&Values::new()).unwrap().len(), 2);
    storage.run("delete", &v(&[("storage", "second")])).ok();
}

fn serde_json_config(id: &str) -> String {
    format!("{{\"id\":\"{id}\",\"kind\":\"local\",\"path\":\"{id}\",\"active\":false}}")
}

#[test]
fn unfinished_uploads_are_listed_and_can_be_aborted() {
    let w = world();
    let files = FileBackend(w.remote.clone());
    let none = files.run("uploadList", &Values::new()).unwrap();
    assert!(none.values.is_empty());
    assert!(
        files
            .run("uploadAbort", &v(&[("uploadId", "nothing")]))
            .is_err()
    );
}
