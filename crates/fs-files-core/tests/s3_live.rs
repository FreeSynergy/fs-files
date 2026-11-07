//! The same suites against a live S3 server (RustFS, MinIO, AWS...). Ignored by default; set
//! `FS_FILES_S3_ENDPOINT`, `FS_FILES_S3_BUCKET`, `FS_FILES_S3_ACCESS_KEY`, `FS_FILES_S3_SECRET_KEY`
//! (the bucket must exist) and run `cargo test --test s3_live -- --ignored`.
#![cfg(feature = "s3")]

mod common;

use std::sync::Arc;

use common::{env, read_all, stream, suite};
use fs_files_core::{Backends, BlobBackend, Files, Transfer};
use serde_json::json;

fn config(prefix: &str) -> serde_json::Value {
    let var = |n: &str| {
        std::env::var(format!("FS_FILES_S3_{n}")).unwrap_or_else(|_| panic!("set FS_FILES_S3_{n}"))
    };
    json!({
        "id": "live", "kind": "s3", "active": true, "prefix": prefix,
        "endpoint": var("ENDPOINT"), "bucket": var("BUCKET"),
        "access_key": var("ACCESS_KEY"), "secret_key": var("SECRET_KEY"),
        "part_size_mib": 5,
    })
}

fn backend(prefix: &str) -> Arc<dyn BlobBackend> {
    Arc::new(
        fs_files_core::S3Config::from_json(&config(prefix))
            .unwrap()
            .connect()
            .unwrap(),
    )
}

fn run_id() -> String {
    format!("test-{}", fs_objects::ObjectId::new())
}

#[tokio::test]
#[ignore = "needs a live S3 server, see the file header"]
async fn the_shared_suite_on_a_live_server() {
    let prefix = run_id();
    let b = backend(&prefix);
    suite::run_all(&*b, "suite").await;
    stream::large(&*b, "big/file", 12_500_000, 700_000).await;
    stream::large(&*b, "tiny/chunks", 6_000_000, 65_536).await;
    stream::resume(
        || async { backend(&prefix) },
        "up/load",
        13_000_000,
        900_000,
    )
    .await;
}

#[tokio::test]
#[ignore = "needs a live S3 server, see the file header"]
async fn a_file_through_the_repository_on_a_live_server() {
    let env = env().await;
    let repo = env.repo.clone();
    let info = repo
        .add_backend(&config(&run_id()).to_string())
        .await
        .unwrap();
    assert_eq!((info.kind.as_str(), info.active), ("s3", true));
    let data = suite::bytes(7_000_000);
    let f = common::upload(&repo, None, "live.bin", "application/octet-stream", &data).await;
    assert_eq!(repo.backend_of(f.id).await.unwrap(), "live");
    assert_eq!(read_all(&repo, f.id).await, data);
    assert_eq!(
        repo.open(f.id)
            .await
            .unwrap()
            .read(6_999_990, 100)
            .await
            .unwrap(),
        data[6_999_990..]
    );
    repo.trash(f.id).await.unwrap();
    repo.purge(f.id).await.unwrap();
    assert!(repo.get(f.id).await.unwrap().is_none());
}
