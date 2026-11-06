//! `FileRepo` with an S3 backend next to a local one: files remember where their bytes are.
#![cfg(feature = "s3")]

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::fake_s3::Fake;
use common::suite::{bytes, checksum};
use common::{env, read_all};
use fs_files_core::{BackendFactory, Backends, BlobBackend, Files, S3Backend, S3Options, Transfer};
use serde_json::Value;

/// Builds an S3 backend over one shared fake bucket, as the real factory would over a live one.
struct FakeBucket(Arc<Fake>);

#[async_trait]
impl BackendFactory for FakeBucket {
    fn kind(&self) -> &'static str {
        "fake-s3"
    }
    async fn build(&self, _: &Value) -> fs_files_core::Result<Arc<dyn BlobBackend>> {
        let opts = S3Options {
            part_size: 5 << 20,
            ..S3Options::default()
        };
        Ok(Arc::new(S3Backend::new(self.0.clone(), &opts)?))
    }
}

#[tokio::test]
async fn files_on_both_backends_stay_readable() {
    let bucket = Arc::new(Fake::default());
    let e = env().await;
    let repo = e
        .repo
        .clone()
        .with_factory(Arc::new(FakeBucket(bucket.clone())));
    let old = common::upload(
        &repo,
        None,
        "local.bin",
        "application/octet-stream",
        b"on disk",
    )
    .await;

    let cfg = r#"{"id":"cloud","kind":"fake-s3","active":true}"#;
    let info = repo.add_backend(cfg).await.unwrap();
    assert_eq!((info.kind.as_str(), info.active), ("s3", true));

    let data = bytes(11_000_000);
    let (up, id) = repo
        .start_upload(None, "cloud.bin", "application/octet-stream")
        .await
        .unwrap();
    for (i, c) in data[..6_000_000].chunks(500_000).enumerate() {
        up.write((i * 500_000) as u64, c).await.unwrap();
    }
    drop(up);
    let up = repo.resume_upload(&id).await.unwrap();
    assert_eq!(up.received().await.unwrap(), 6_000_000);
    up.write(6_000_000, &data[6_000_000..]).await.unwrap();
    let new = up.finish().await.unwrap();
    assert_eq!(new.checksum, checksum(&data));

    assert_eq!(repo.backend_of(old.id).await.unwrap(), "main");
    assert_eq!(repo.backend_of(new.id).await.unwrap(), "cloud");
    assert_eq!(read_all(&repo, old.id).await, b"on disk");
    assert_eq!(read_all(&repo, new.id).await, data);
    let blob = repo.open(new.id).await.unwrap();
    assert_eq!(
        blob.read(5_242_870, 30).await.unwrap(),
        data[5_242_870..5_242_900],
        "across a part border"
    );

    let copy = repo.copy_file(new.id, None).await.unwrap();
    assert_eq!(copy.checksum, new.checksum);
    repo.trash(new.id).await.unwrap();
    repo.purge(new.id).await.unwrap();
    repo.trash(copy.id).await.unwrap();
    repo.purge(copy.id).await.unwrap();
    assert!(
        bucket.keys().is_empty(),
        "purge deleted the bytes: {:?}",
        bucket.keys()
    );
    repo.remove_backend("cloud")
        .await
        .expect_err("the active backend stays");
}
