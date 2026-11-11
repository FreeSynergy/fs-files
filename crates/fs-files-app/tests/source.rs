//! `RemoteFileSource`, the source of the file window, over a real server, repository and folder.
//!
//! Pattern: Integration test — the same server as in `backend.rs`; the source is the one the window uses.

mod support;

use fs_components_workbenches::file_manager::{
    EntryKind, FileProblem, FileSource, ListFilter, ListRequest, NameRule, Scope, SortKey,
    SortOrder,
};
use fs_files_app::RemoteFileSource;
use support::world;

fn all(scope: Scope, term: &str) -> ListRequest {
    ListRequest::new(scope, term, ListFilter::default(), SortOrder::default())
}

fn names(source: &RemoteFileSource, request: &ListRequest) -> Vec<String> {
    source
        .list(request, None)
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.name)
        .collect()
}

#[test]
// One story from upload to purge: split, it would lose the thread.
#[allow(clippy::too_many_lines)]
fn the_window_source_lists_changes_trashes_restores_and_downloads() {
    let w = world();
    let source = RemoteFileSource(w.remote.clone());
    let local = w.dir.join("note.txt");
    std::fs::write(&local, "hello").unwrap();

    let docs = source.create_folder(None, "Docs").unwrap();
    assert_eq!(docs.name, "Docs");
    let name = source
        .upload(local.to_str().unwrap(), Some(&docs.key), NameRule::Refuse)
        .unwrap();
    assert_eq!(name, "note.txt");

    // the top has the folder, the folder has the file, with its facts
    assert_eq!(names(&source, &all(Scope::Folder(None), "")), ["Docs"]);
    let page = source
        .list(&all(Scope::Folder(Some(docs.key.clone())), ""), None)
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    let file = page.entries[0].clone();
    assert_eq!((file.kind, file.size), (EntryKind::File, 5));
    assert!(file.modified.is_some());
    assert_eq!(source.folder(&docs.key).unwrap().unwrap().name, "Docs");
    assert_eq!(
        source.subfolders(None, 10).unwrap().0[0].key,
        docs.key
    );

    // the search finds it everywhere; a sort by size answers too
    assert_eq!(names(&source, &all(Scope::Everywhere, "note")), ["note.txt"]);
    let by_size = ListRequest::new(
        Scope::Folder(Some(docs.key.clone())),
        "",
        ListFilter::default(),
        SortOrder {
            key: SortKey::Size,
            descending: true,
        },
    );
    assert_eq!(names(&source, &by_size), ["note.txt"]);

    // preview and rename
    let bytes = source.preview(&file.key, 3).unwrap();
    assert_eq!((bytes.bytes.as_slice(), bytes.total), (&b"hel"[..], 5));
    assert_eq!(
        source
            .rename(&file.reference(), "gruss.txt", NameRule::Refuse)
            .unwrap(),
        "gruss.txt"
    );

    // a taken name is a name conflict, not "something failed"
    let again = w.dir.join("gruss.txt");
    std::fs::write(&again, "x").unwrap();
    let e = source
        .upload(again.to_str().unwrap(), Some(&docs.key), NameRule::Refuse)
        .unwrap_err();
    assert_eq!(e.problem, FileProblem::NameConflict);

    // trash, count, restore
    source.trash(&file.key).unwrap();
    assert!(names(&source, &all(Scope::Folder(Some(docs.key.clone())), "")).is_empty());
    assert_eq!(names(&source, &all(Scope::Trash, "")), ["gruss.txt"]);
    assert_eq!(source.trash_count().unwrap().files, 1);
    assert_eq!(
        source.restore(&file.key, NameRule::Refuse).unwrap(),
        "gruss.txt"
    );

    // download never replaces
    let out = w.dir.join("back.txt");
    source
        .download(&file.key, out.to_str().unwrap(), false)
        .unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "hello");
    assert!(source
        .download(&file.key, out.to_str().unwrap(), false)
        .is_err());

    // a file moves to the top and a folder goes when it is empty
    source
        .move_to(&file.reference(), None, NameRule::Refuse)
        .unwrap();
    source.delete_folder(&docs.key).unwrap();
    assert_eq!(names(&source, &all(Scope::Folder(None), "")), ["gruss.txt"]);
}
