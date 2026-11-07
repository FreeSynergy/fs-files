//! Sensitive categories (ADR 0029): they go to the envelope; the store decides if it may hold them.

mod common;

use common::{env, reopen, upload};
use fs_files_core::{Error, Files};

#[tokio::test]
async fn categories_go_to_the_envelope() {
    let env = env().await;
    let f = upload(&env.repo, None, "tax.pdf", "application/pdf", b"%PDF").await;
    let c = env
        .repo
        .classify(
            f.id,
            vec![
                "dpv:Financial".into(),
                "dpv:Health".into(),
                "dpv:Financial".into(),
            ],
        )
        .await
        .unwrap();
    assert_eq!(c.sensitive, ["dpv:Financial", "dpv:Health"]);
    let stored = env.store.get(f.id).await.unwrap().unwrap();
    assert_eq!(stored.envelope.sensitive, ["dpv:Financial", "dpv:Health"]);
    // a copy stays sensitive
    let copy = env.repo.copy_file(f.id, None).await.unwrap();
    assert_eq!(copy.sensitive.len(), 2);
    let cleared = env.repo.classify(f.id, vec![]).await.unwrap();
    assert!(cleared.sensitive.is_empty());
}

#[tokio::test]
async fn an_unencrypted_store_refuses_and_the_file_stays_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, _) = reopen(dir.path(), false).await;
    let f = common::upload(&repo, None, "tax.pdf", "application/pdf", b"%PDF").await;
    let err = repo
        .classify(f.id, vec!["dpv:Financial".into()])
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Store(fs_objects::Error::EncryptionRequired(_))),
        "{err}"
    );
    assert!(repo.get(f.id).await.unwrap().unwrap().sensitive.is_empty());
}

#[tokio::test]
async fn an_encrypted_store_accepts() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, _) = reopen(dir.path(), true).await;
    let f = common::upload(&repo, None, "tax.pdf", "application/pdf", b"%PDF").await;
    let c = repo
        .classify(f.id, vec!["dpv:Financial".into()])
        .await
        .unwrap();
    assert_eq!(c.sensitive, ["dpv:Financial"]);
}
