//! A file larger than a chunk is never held whole and is read back in ranges.
#![allow(clippy::cast_possible_truncation, clippy::used_underscore_binding)]
// tests: small known test sizes, truncation cannot happen; tests: `_name` fixtures are kept alive on purpose and read by the harness
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::env;
use common::suite::bytes;
use fs_files_core::Transfer;

#[tokio::test]
async fn a_file_larger_than_a_chunk_is_read_back_in_ranges() {
    let _env = env().await;
    let e = _env.repo.clone();
    let data = bytes(5_300_000); // > 5 chunks of 1 MiB
    let (up, _) = e
        .start_upload(None, "large.bin", "application/octet-stream")
        .await
        .unwrap();
    for (i, c) in data.chunks(777_777).enumerate() {
        up.write((i * 777_777) as u64, c).await.unwrap();
    }
    let f = up.finish().await.unwrap();
    assert_eq!(f.size, data.len() as u64);
    let blob = e.open(f.id).await.unwrap();
    assert_eq!(blob.size().await.unwrap().0, data.len() as u64);
    for (offset, len) in [
        (0usize, 10usize),
        (1_048_570, 20),
        (2_000_000, 1_500_000),
        (5_299_990, 100),
    ] {
        let got = blob.read(offset as u64, len as u32).await.unwrap();
        assert_eq!(
            got,
            data[offset..(offset + len).min(data.len())],
            "{offset}"
        );
    }
    assert!(blob.read(9_000_000, 10).await.unwrap().is_empty());
}
