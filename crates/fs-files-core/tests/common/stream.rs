//! Streaming checks shared by every backend: a blob larger than a chunk, and a restart in the middle.

use std::future::Future;
use std::sync::Arc;

use fs_files_core::{BlobBackend, Error};

use super::suite::{bytes, checksum};

/// A blob larger than `total` bytes, written in `chunk`-sized puts, read back in ranges that do
/// not line up with the chunks. Memory use of the backend must not depend on `total`.
pub async fn large(b: &dyn BlobBackend, key: &str, total: usize, chunk: usize) {
    let data = bytes(total);
    for (i, piece) in data.chunks(chunk).enumerate() {
        b.put(key, (i * chunk) as u64, piece).await.unwrap();
    }
    let info = b.commit(key).await.unwrap();
    assert_eq!(info.size, total as u64);
    assert_eq!(info.checksum, checksum(&data));
    let mut offset = 0usize;
    let mut step = 1_000_003usize;
    while offset < total {
        let got = b.get(key, offset as u64, step as u32).await.unwrap();
        assert_eq!(
            got,
            data[offset..(offset + step).min(total)],
            "range at {offset}"
        );
        offset += step;
        step = step * 3 / 2 + 7;
    }
    b.delete(key).await.unwrap();
}

/// A restart in the middle of an upload: `open` builds a fresh backend over the same storage.
/// The pending bytes survive, writing continues at the right offset, the checksum is correct.
pub async fn resume<F, Fut>(open: F, key: &str, total: usize, chunk: usize)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Arc<dyn BlobBackend>>,
{
    let data = bytes(total);
    let half = (total / 2 / chunk) * chunk;
    let first = open().await;
    for (i, piece) in data[..half].chunks(chunk).enumerate() {
        first.put(key, (i * chunk) as u64, piece).await.unwrap();
    }
    drop(first);
    let second = open().await;
    assert_eq!(
        second.pending(key).await.unwrap(),
        half as u64,
        "state survived the restart"
    );
    assert!(matches!(
        second.put(key, 0, b"x").await,
        Err(Error::Offset { .. })
    ));
    for (i, piece) in data[half..].chunks(chunk).enumerate() {
        second
            .put(key, (half + i * chunk) as u64, piece)
            .await
            .unwrap();
    }
    let info = second.commit(key).await.unwrap();
    assert_eq!(info.size, total as u64);
    assert_eq!(info.checksum, checksum(&data), "checksum after a restart");
    assert_eq!(
        second.get(key, (half - 3) as u64, 10).await.unwrap(),
        data[half - 3..half + 7]
    );
    second.delete(key).await.unwrap();
}
