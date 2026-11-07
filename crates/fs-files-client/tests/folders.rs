//! Folders and files over the socket: what the client reads is what the provider holds, and the
//! problems of the server come back as the errors of the core.
//!
//! Integration tests against the traits; nothing stands in for the server or the store.

mod common;

use common::Served;
use fs_files_core::{Entry, Error, File, Files, OnConflict, Sort, SortBy};
use fs_interfaces::files_capnp::{INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE};
use fs_store::ObjectId;

fn same(a: &File, b: &File) {
    assert_eq!(
        (a.id, &a.iri, &a.name, &a.media_type, a.size, &a.checksum),
        (b.id, &b.iri, &b.name, &b.media_type, b.size, &b.checksum)
    );
    assert_eq!(
        (a.folder, a.trashed, &a.sensitive),
        (b.folder, b.trashed, &b.sensitive)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_and_a_folder_read_as_the_provider_holds_them() {
    let served = Served::start().await;
    let docs = served.folder(None, "Docs").await;
    let made = served.file(Some(docs.id), "a.txt", b"hello").await;
    let files = served.client();
    let got = files.get(made.id).await.unwrap().expect("found");
    same(&got, &made);
    let folder = files.get_folder(docs.id).await.unwrap().expect("found");
    assert_eq!(
        (folder.id, &folder.name, folder.parent),
        (docs.id, &docs.name, None)
    );
    assert!(files.get(ObjectId::new()).await.unwrap().is_none());
    assert!(files.get_folder(ObjectId::new()).await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn folders_and_files_change_through_the_client() {
    let served = Served::start().await;
    let files = served.client();
    let docs = files.create_folder(None, "Docs").await.unwrap();
    let inner = files.create_folder(Some(docs.id), "Inner").await.unwrap();
    assert_eq!(inner.parent, Some(docs.id));
    let renamed = files
        .rename_folder_as(inner.id, "Deep", OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(renamed.name, "Deep");
    let moved = files
        .move_folder_as(renamed.id, None, OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(moved.parent, None);
    let made = served.file(None, "a.txt", b"bytes").await;
    let renamed = files
        .rename_file_as(made.id, "b.txt", OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(renamed.name, "b.txt");
    let moved = files
        .move_file_as(made.id, Some(docs.id), OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(moved.folder, Some(docs.id));
    let copy = files
        .copy_file_as(made.id, None, OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_ne!(copy.id, made.id);
    assert_eq!(copy.size, 5);
    let on_server = served.repo.get(copy.id).await.unwrap().unwrap();
    same(&copy, &on_server);
}

#[tokio::test(flavor = "multi_thread")]
async fn trash_restore_purge_and_empty() {
    let served = Served::start().await;
    let files = served.client();
    let a = served.file(None, "a.txt", b"a").await;
    let b = served.file(None, "b.txt", b"b").await;
    let c = served.file(None, "c.txt", b"c").await;
    files.trash(a.id).await.unwrap();
    assert!(files.get(a.id).await.unwrap().unwrap().trashed);
    let back = files.restore_as(a.id, OnConflict::Refuse).await.unwrap();
    assert!(!back.trashed);
    files.trash(b.id).await.unwrap();
    files.purge(b.id).await.unwrap();
    assert!(files.get(b.id).await.unwrap().is_none());
    let folder = served.folder(None, "Elsewhere").await;
    files.trash(c.id).await.unwrap();
    let there = files
        .restore_to(c.id, Some(folder.id), OnConflict::Refuse)
        .await
        .unwrap();
    assert_eq!(there.folder, Some(folder.id));
    files.trash(a.id).await.unwrap();
    let report = files.empty_trash().await.unwrap();
    assert_eq!((report.purged, report.failed.len()), (1, 0));
    files.delete_folder(folder.id).await.unwrap_err();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_taken_name_is_the_named_problem_and_keep_both_gives_a_free_one() {
    let served = Served::start().await;
    let files = served.client();
    served.file(None, "a.txt", b"one").await;
    let two = served.file(None, "b.txt", b"two").await;
    let err = files
        .rename_file_as(two.id, "a.txt", OnConflict::Refuse)
        .await
        .unwrap_err();
    let named = err.problem().expect("a named problem");
    assert_eq!(named.kind, NAME_CONFLICT_PROBLEM_TYPE, "{err:?}");
    let kept = files
        .rename_file_as(two.id, "a.txt", OnConflict::KeepBoth)
        .await
        .unwrap();
    assert_eq!(kept.name, "a (2).txt");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_name_and_a_missing_id_come_back_as_what_they_are() {
    let served = Served::start().await;
    let files = served.client();
    let err = files.create_folder(None, "a/b").await.unwrap_err();
    assert_eq!(
        err.problem().unwrap().kind,
        INVALID_NAME_PROBLEM_TYPE,
        "{err:?}"
    );
    let err = files.trash(ObjectId::new()).await.unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sorted_list_comes_in_the_order_asked_for() {
    let served = Served::start().await;
    let files = served.client();
    served.file(None, "m.txt", b"12").await;
    served.file(None, "a.txt", b"1234567").await;
    served.file(None, "z.txt", b"1").await;
    let by = |by, descending| Sort { by, descending };
    let names = |page: fs_files_core::Page| -> Vec<String> {
        page.items
            .iter()
            .map(|e| match e {
                Entry::File(f) => f.name.clone(),
                Entry::Folder(f) => f.name.clone(),
            })
            .collect()
    };
    let page = files
        .list_sorted(None, None, by(SortBy::Size, true))
        .await
        .unwrap();
    assert_eq!(names(page), ["a.txt", "m.txt", "z.txt"]);
    let page = files
        .list_sorted(None, None, by(SortBy::Name, false))
        .await
        .unwrap();
    assert_eq!(names(page), ["a.txt", "m.txt", "z.txt"]);
}
