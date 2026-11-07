//! Looking at the folder, and what the client says it can do.

mod common;

use common::Served;
use fs_files_core::{Entry, Files};

#[tokio::test(flavor = "multi_thread")]
async fn a_file_put_in_the_folder_from_outside_is_taken_over_by_a_rescan() {
    let served = Served::start().await;
    let files = served.client();
    std::fs::write(served.root.join("outside.txt"), b"hello").unwrap();
    let report = files.rescan(None).await.unwrap();
    assert_eq!(report.added.len(), 1);
    assert!(matches!(&report.added[0], Entry::File(f) if f.name == "outside.txt" && f.size == 5));
    assert!(report.updated.is_empty() && report.missing.is_empty() && report.skipped.is_empty());
    std::fs::remove_file(served.root.join("outside.txt")).unwrap();
    let report = files.rescan(None).await.unwrap();
    assert_eq!(
        report.missing.len(),
        1,
        "a vanished file is reported, its record kept"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_client_reports_only_what_it_can_do() {
    let served = Served::start().await;
    let files = served.client();
    let caps = files.capabilities();
    for id in [
        "file.resolve",
        "folder.list",
        "folder.create",
        "file.upload",
        "file.download",
        "file.pendingUploads",
        "folder.rescan",
        "file.search",
        "file.sort",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
    for id in [
        "file.classify",
        "file.subscribe",
        "backend.list",
        "backend.add",
        "backend.remove",
    ] {
        assert!(!caps.is_supported(id), "{id} is not called by this client");
    }
    assert!(!caps.is_supported("share.create"), "the server says no");
}

#[tokio::test(flavor = "multi_thread")]
async fn what_the_client_does_not_call_answers_unsupported() {
    let served = Served::start().await;
    let files = served.client();
    let made = served.file(None, "a.txt", b"x").await;
    let err = files
        .classify(made.id, vec!["health".into()])
        .await
        .unwrap_err();
    assert!(
        matches!(err, fs_files_core::Error::Unsupported(_)),
        "{err:?}"
    );
    let err = files.sharing().await.unwrap_err();
    assert!(
        matches!(err, fs_files_core::Error::Unsupported(_)),
        "{err:?}"
    );
}
