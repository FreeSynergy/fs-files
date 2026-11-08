//! The S3 backend: the shared suite on a strict fake and on the `object_store` in-memory store, then
//! what is specific to S3 (part sizes, restart, leftovers). A live server is in `s3_live.rs`.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons
#![cfg(feature = "s3")]

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::fake_s3::Fake;
use common::{stream, suite};
use fs_files_core::{BlobBackend, ObjectApi, ObjectStoreApi, S3Backend, S3Options};

fn backend(api: &Arc<dyn ObjectApi>) -> Arc<dyn BlobBackend> {
    Arc::new(
        S3Backend::new(
            api.clone(),
            &S3Options {
                prefix: "fs".into(),
                part_size: 5 << 20,
            },
        )
        .unwrap(),
    )
}

fn fake() -> (Arc<Fake>, Arc<dyn ObjectApi>) {
    let f = Arc::new(Fake::default());
    (f.clone(), f)
}

fn in_memory() -> Arc<dyn ObjectApi> {
    Arc::new(ObjectStoreApi(object_store::memory::InMemory::new()))
}

#[tokio::test]
async fn passes_the_shared_suite_on_the_strict_fake() {
    let (_, api) = fake();
    suite::run_all(&*backend(&api), "suite").await;
}

#[tokio::test]
async fn passes_the_shared_suite_through_the_object_store_adapter() {
    suite::run_all(&*backend(&in_memory()), "suite").await;
}

#[tokio::test]
async fn a_large_file_streams_through_multipart() {
    let (f, api) = fake();
    // 12.5 MB in 700 KB chunks: three parts of at least 5 MiB, no memory for the whole file.
    stream::large(&*backend(&api), "big/file", 12_500_000, 700_000).await;
    assert!(f.multiparts_started.load(Ordering::SeqCst) >= 1);
    assert_eq!(f.keys(), Vec::<String>::new(), "no leftovers after delete");
    stream::large(&*backend(&in_memory()), "big/file", 12_500_000, 700_000).await;
}

#[tokio::test]
async fn tiny_chunks_still_make_valid_parts() {
    let (f, api) = fake();
    let b = backend(&api);
    // 64 KiB chunks are far under S3's 5 MiB minimum; they are collected before a part is sent.
    stream::large(&*b, "small/chunks", 11_000_000, 65_536).await;
    assert_eq!(f.keys(), Vec::<String>::new());
}

#[tokio::test]
async fn a_small_file_needs_no_multipart_at_all() {
    let (f, api) = fake();
    let b = backend(&api);
    b.put("k/small", 0, &suite::bytes(100_000)).await.unwrap();
    b.commit("k/small").await.unwrap();
    assert_eq!(f.multiparts_started.load(Ordering::SeqCst), 0);
    assert_eq!(f.keys(), ["fs/data/k/small"], "pending objects are gone");
}

#[tokio::test]
async fn an_upload_resumes_after_a_restart() {
    let (f, api) = fake();
    let open = || {
        let api = api.clone();
        async move { backend(&api) }
    };
    stream::resume(open, "up/load", 13_000_000, 900_000).await;
    assert!(f.multiparts_started.load(Ordering::SeqCst) >= 1);
    let (_, mem) = (0, in_memory());
    let open = || {
        let api = mem.clone();
        async move { backend(&api) }
    };
    stream::resume(open, "up/load", 13_000_000, 900_000).await;
}

#[tokio::test]
async fn abort_removes_parts_pieces_and_the_multipart_upload() {
    let (f, api) = fake();
    let b = backend(&api);
    let data = suite::bytes(6_000_000);
    b.put("a/b", 0, &data).await.unwrap(); // one part sent
    b.put("a/b", 6_000_000, &data[..1000]).await.unwrap(); // and a tail piece
    assert_eq!(f.open_uploads(), 1);
    assert!(!f.keys().is_empty());
    b.delete("a/b").await.unwrap();
    assert_eq!(f.open_uploads(), 0);
    assert_eq!(f.keys(), Vec::<String>::new());
}

#[tokio::test]
async fn a_crash_between_part_and_cleanup_loses_nothing() {
    let (f, api) = fake();
    let b = backend(&api);
    let data = suite::bytes(9_000_000);
    b.put("c/rash", 0, &data[..4_000_000]).await.unwrap();
    f.fail_deletes.store(true, Ordering::SeqCst);
    // this chunk completes a part; the cleanup of the pieces fails after the part was saved
    assert!(
        b.put("c/rash", 4_000_000, &data[4_000_000..6_000_000])
            .await
            .is_err()
    );
    f.fail_deletes.store(false, Ordering::SeqCst);
    drop(b);
    let b = backend(&api); // a restart: the state is rebuilt from the bucket
    assert_eq!(
        b.pending("c/rash").await.unwrap(),
        6_000_000,
        "the bytes are durable, no double count"
    );
    b.put("c/rash", 6_000_000, &data[6_000_000..])
        .await
        .unwrap();
    let info = b.commit("c/rash").await.unwrap();
    assert_eq!(
        info.checksum,
        suite::checksum(&data),
        "stale pieces were not used twice"
    );
    assert_eq!(
        b.get("c/rash", 5_999_990, 20).await.unwrap(),
        data[5_999_990..6_000_010]
    );
}

#[tokio::test]
async fn writing_without_a_start_at_zero_creates_nothing() {
    let (f, api) = fake();
    assert!(backend(&api).put("n/ew", 7, b"x").await.is_err());
    assert!(f.keys().is_empty());
}

#[test]
fn the_part_size_has_the_s3_minimum() {
    let api = in_memory();
    let small = S3Options {
        part_size: 4 << 20,
        ..S3Options::default()
    };
    assert!(S3Backend::new(api.clone(), &small).is_err());
    let bad_prefix = S3Options {
        prefix: "../x".into(),
        ..S3Options::default()
    };
    assert!(S3Backend::new(api, &bad_prefix).is_err());
}
