//! Finishing the same upload twice at once makes one file, not two records on one object.

mod common;

use common::{env, read_all};
use fs_files_core::{Files, OnConflict, Transfer};

async fn twice(r: &fs_files_core::FileRepo, on: OnConflict) {
    let tag = format!("{on:?}");
    for round in 0..10 {
        let name = format!("{tag}n{round}.bin");
        let (a, id) = r
            .start_upload_as(None, &name, "application/octet-stream", on)
            .await
            .unwrap();
        a.write(0, b"one object").await.unwrap();
        let b = r.resume_upload(&id).await.unwrap();
        let (x, y) = tokio::join!(a.finish(), b.finish());
        let (x, y) = (x.unwrap(), y.unwrap());
        assert_eq!(
            x.id, y.id,
            "round {round}: both finishes return the same file"
        );
        let count = r
            .find(
                &fs_files_core::FileFilter {
                    name: Some(format!("{tag}n{round}.")),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap()
            .items
            .len();
        assert_eq!(count, 1, "round {round}: one record");
        // purging it must not touch anything else
        let keep = common::upload(
            r,
            None,
            &format!("{tag}other{round}"),
            "text/plain",
            b"other",
        )
        .await;
        r.trash(x.id).await.unwrap();
        r.purge(x.id).await.unwrap();
        assert_eq!(read_all(r, keep.id).await, b"other");
    }
}

#[tokio::test]
async fn two_finishes_at_once_on_the_local_disk() {
    let e = env().await;
    twice(&e.repo, OnConflict::KeepBoth).await;
    twice(&e.repo, OnConflict::Refuse).await;
}

#[cfg(feature = "s3")]
#[tokio::test]
async fn two_finishes_at_once_on_s3() {
    use common::fake_s3::Fake;
    use fs_files_core::{S3Backend, S3Options};
    use std::sync::Arc;
    let store: Arc<dyn fs_store::Store> =
        Arc::new(fs_store::TursoStore::open_in_memory().await.unwrap());
    let backend =
        Arc::new(S3Backend::new(Arc::new(Fake::default()), &S3Options::default()).unwrap());
    let r = fs_files_core::FileRepo::new(store, common::ME, "cloud", backend);
    twice(&r, OnConflict::KeepBoth).await;
    twice(&r, OnConflict::Refuse).await;
}
