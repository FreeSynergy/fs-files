//! The support report names the store of the provider (`SupportReport.instance`), and only there.

mod common;

use std::sync::Arc;

use common::stub::Stub;
use common::{ME, text_file, with_repo, with_stub};
use fs_files_core::{FileRepo, LocalFolder};
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::read_uuid;
use fs_store::{ActivityFilter, Store, TursoStore};
use uuid::Uuid;

async fn instance_of(root: &files::Client) -> Option<Uuid> {
    let answer = root.support_report_request().send().promise.await.unwrap();
    let report = answer.get().unwrap().get_report().unwrap();
    read_uuid(report.get_instance().unwrap())
}

async fn repo_over(store: Arc<dyn Store>, dir: &std::path::Path) -> Arc<FileRepo> {
    let backend = Arc::new(LocalFolder::open(dir.join("files")).await.unwrap());
    Arc::new(FileRepo::new(store, ME, "main", backend))
}

#[tokio::test(flavor = "current_thread")]
async fn the_report_carries_the_id_of_the_store_and_the_log_does_not() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let repo = repo_over(store.clone(), dir.path()).await;
            with_repo(repo, |root| async move {
                let c = common::caps(&root).await;
                let told = instance_of(&root)
                    .await
                    .expect("the report names an instance");
                assert_eq!(told, store.identity().as_uuid());
                assert_eq!(instance_of(&root).await, Some(told));
                text_file(&c, None, "a.txt", "x").await;
                let log = store.activities(&ActivityFilter::default()).await.unwrap();
                assert!(!log.is_empty());
                let written = format!("{log:?}");
                assert!(!written.contains(&told.to_string()), "{written}");
            })
            .await;
        })
        .await;
}

async fn serve_file(path: &std::path::Path) -> (Uuid, Uuid) {
    let store: Arc<dyn Store> = Arc::new(TursoStore::open(path).await.unwrap());
    let repo = repo_over(store.clone(), path.parent().unwrap()).await;
    let told = std::rc::Rc::new(std::cell::Cell::new(None));
    let slot = told.clone();
    with_repo(repo, |root| async move {
        slot.set(instance_of(&root).await);
    })
    .await;
    (told.get().unwrap(), store.identity().as_uuid())
}

#[tokio::test(flavor = "current_thread")]
async fn the_id_stays_over_a_restart_and_a_new_store_has_another() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let (first, stored) = serve_file(&dir.path().join("a.db")).await;
            assert_eq!(first, stored);
            let (again, _) = serve_file(&dir.path().join("a.db")).await;
            assert_eq!(again, first);
            let (other, _) = serve_file(&dir.path().join("b.db")).await;
            assert_ne!(other, first);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_provider_without_a_store_leaves_the_instance_empty() {
    with_stub(Stub::default(), |root| async move {
        assert_eq!(instance_of(&root).await, None);
    })
    .await;
}
