//! The file manager's source over the socket: `FilesRemote` against a real server, repository and folder.
//!
//! Pattern: Integration test — the server runs on its own thread and socket; the source is the one the
//! window uses.

use std::path::Path;
use std::sync::Arc;

use fs_components_workbenches::file_manager::{
    EntryKind, EntryRef, FileProblem, FileSource, ListFilter, ListRequest, NameRule, Scope,
    SortOrder,
};
use fs_files_app::FilesRemote;
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

fn request(scope: Scope, term: &str) -> ListRequest {
    ListRequest::new(scope, term, ListFilter::default(), SortOrder::default())
}

fn names(source: &FilesRemote, scope: Scope, term: &str) -> Vec<String> {
    source
        .list(&request(scope, term), None)
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.name)
        .collect()
}

#[test]
fn folders_rename_move_trash_restore_and_search_over_the_socket() {
    let dir = tempfile::tempdir().unwrap();
    let (socket, root) = (dir.path().join("files.sock"), dir.path().join("files"));
    start_server(&socket, &root);
    let source = FilesRemote::connect(&socket).unwrap();

    let docs = source.create_folder(None, "Documents").unwrap();
    let archive = source.create_folder(None, "Archive").unwrap();
    assert_eq!(
        names(&source, Scope::Folder(None), ""),
        ["Archive", "Documents"]
    );
    let (subs, more) = source.subfolders(None, 10).unwrap();
    assert_eq!((subs.len(), more), (2, false));
    assert_eq!(source.folder(&docs.key).unwrap().unwrap().name, "Documents");

    // A file on the disk is taken over by a rescan, then it can be worked with.
    std::fs::write(root.join("note.txt"), "hello").unwrap();
    let summary = source.rescan(None).unwrap();
    assert!(
        summary.added.contains(&"note.txt".to_string()),
        "{summary:?}"
    );
    let note = source
        .list(&request(Scope::Folder(None), "note"), None)
        .unwrap()
        .entries
        .remove(0);
    assert_eq!((note.kind, note.size), (EntryKind::File, 5));

    let moved = source
        .move_to(&note.reference(), Some(&docs.key), NameRule::Refuse)
        .unwrap();
    assert_eq!(
        names(&source, Scope::Folder(Some(docs.key.clone())), ""),
        ["note.txt"]
    );
    source
        .rename(
            &EntryRef {
                key: moved.clone(),
                kind: EntryKind::File,
            },
            "memo.txt",
            NameRule::Refuse,
        )
        .unwrap();
    assert_eq!(names(&source, Scope::Everywhere, "memo"), ["memo.txt"]);

    // A taken name is the workbench's own kind of problem, with the server's words.
    source.create_folder(Some(&docs.key), "Sub").unwrap();
    let clash = source.create_folder(Some(&docs.key), "Sub").unwrap_err();
    assert_eq!(clash.problem, FileProblem::NameConflict, "{clash}");

    source.trash(&moved).unwrap();
    assert_eq!(names(&source, Scope::Trash, ""), ["memo.txt"]);
    assert_eq!(source.trash_count().unwrap().files, 1);
    source
        .restore_to(&moved, Some(&archive.key), NameRule::Refuse)
        .unwrap();
    assert_eq!(
        names(&source, Scope::Folder(Some(archive.key.clone())), ""),
        ["memo.txt"]
    );
    assert!(source.trash_count().unwrap().files == 0);

    // What this window does not do says so.
    assert_eq!(
        source.preview(&moved, 10).unwrap_err().problem,
        FileProblem::Unsupported
    );
    assert!(!source.capabilities().upload);
}

#[test]
fn nothing_on_the_socket_is_said_plainly() {
    let dir = tempfile::tempdir().unwrap();
    let message = FilesRemote::connect(&dir.path().join("none.sock"))
        .err()
        .unwrap();
    assert!(message.contains("fs-files serve"), "{message}");
}
