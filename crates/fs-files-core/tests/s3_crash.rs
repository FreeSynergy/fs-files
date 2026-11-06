//! A crash at every step of `commit` on S3 leaves an upload that can be finished or dropped, and a
//! drop never deletes a finished object.
#![cfg(feature = "s3")]

mod common;

use std::sync::Arc;

use common::fake_s3::Fake;
use common::suite::{bytes, checksum};
use fs_files_core::{BlobBackend, S3Backend, S3Options, Spot};

fn backend(bucket: &Arc<Fake>) -> S3Backend {
    S3Backend::new(
        bucket.clone(),
        &S3Options {
            part_size: 5 << 20,
            ..S3Options::default()
        },
    )
    .unwrap()
}

/// Write `data` in chunks, then crash at the n-th call of `commit`.
async fn crashed_commit(bucket: &Arc<Fake>, key: &str, data: &[u8], n: usize) -> bool {
    let b = backend(bucket);
    for (i, c) in data.chunks(1_000_000).enumerate() {
        b.put(key, (i * 1_000_000) as u64, c).await.unwrap();
    }
    bucket.crash_at(n);
    let r = b.commit(key).await;
    if !bucket.crashed() {
        assert!(r.is_ok(), "no crash happened, so commit must succeed");
        bucket.restart();
        return false;
    }
    assert!(r.is_err());
    bucket.restart();
    true
}

async fn every_step(data: Vec<u8>) {
    let mut n = 1;
    loop {
        let bucket = Arc::new(Fake::default());
        let key = "ab/obj";
        if !crashed_commit(&bucket, key, &data, n).await {
            assert!(n > 3, "commit has several steps");
            return;
        }
        // restart: a new backend over the same bucket, retry the commit
        let b = backend(&bucket);
        let info = b
            .commit(key)
            .await
            .unwrap_or_else(|e| panic!("crash at call {n}: retry failed: {e}"));
        assert_eq!(info.size, data.len() as u64, "crash at call {n}");
        assert_eq!(info.checksum, checksum(&data), "crash at call {n}");
        assert_eq!(
            b.get(key, 0, u32::MAX).await.unwrap(),
            data,
            "crash at call {n}"
        );
        assert_eq!(
            bucket.keys().len(),
            1,
            "crash at call {n}: only the object is left: {:?}",
            bucket.keys()
        );
        assert_eq!(bucket.open_uploads(), 0, "crash at call {n}");
        n += 1;
    }
}

#[tokio::test]
async fn a_multipart_commit_survives_a_crash_at_every_call() {
    every_step(bytes(7_000_000)).await;
}

#[tokio::test]
async fn a_plain_put_commit_survives_a_crash_at_every_call() {
    every_step(bytes(3_000)).await;
}

#[tokio::test]
async fn dropping_the_pending_upload_after_a_crash_never_deletes_a_finished_object() {
    for data in [bytes(7_000_000), bytes(3_000)] {
        let mut n = 1;
        loop {
            let bucket = Arc::new(Fake::default());
            let key = "ab/obj";
            if !crashed_commit(&bucket, key, &data, n).await {
                break;
            }
            let object_before = bucket.object("ab/obj");
            let b = backend(&bucket);
            b.discard(&Spot::Pending(key.into()))
                .await
                .unwrap_or_else(|e| panic!("call {n}: {e}"));
            assert_eq!(
                bucket.object("ab/obj"),
                object_before,
                "call {n}: the object is untouched"
            );
            assert_eq!(
                b.pending(key).await.unwrap(),
                0,
                "call {n}: nothing pending"
            );
            assert_eq!(bucket.open_uploads(), 0, "call {n}: no multipart left");
            n += 1;
        }
    }
}

#[tokio::test]
async fn an_older_object_of_the_key_is_replaced_not_mistaken_for_the_new_one() {
    let bucket = Arc::new(Fake::default());
    let b = backend(&bucket);
    b.put("k", 0, b"old-old").await.unwrap();
    b.commit("k").await.unwrap();
    b.put("k", 0, b"new-new").await.unwrap(); // same size
    bucket.crash_at(2); // after the old object was removed or before: either way the retry decides right
    let _ = b.commit("k").await;
    bucket.restart();
    let b2 = backend(&bucket);
    let info = b2.commit("k").await.unwrap();
    assert_eq!(info.checksum, checksum(b"new-new"));
    assert_eq!(b2.get("k", 0, 99).await.unwrap(), b"new-new");
}
