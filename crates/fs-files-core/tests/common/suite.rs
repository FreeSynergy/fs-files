//! One behaviour suite for every `BlobBackend` (ADR 0045: two providers, one behaviour).
//! Each backend's test file calls these functions; they cannot drift apart.

use fs_files_core::{BlobBackend, Error, format_checksum};

/// Deterministic bytes that are not periodic at chunk size.
pub fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * 31 + i / 251) % 256) as u8).collect()
}

pub fn checksum(data: &[u8]) -> String {
    format_checksum(&blake3::hash(data))
}

/// Every check that needs only a backend. `unique` makes keys that do not collide between runs.
pub async fn run_all(b: &dyn BlobBackend, unique: &str) {
    capabilities(b);
    roundtrip(b, &format!("{unique}/round")).await;
    wrong_offset(b, &format!("{unique}/offset")).await;
    empty_and_overwrite(b, &format!("{unique}/empty")).await;
    delete_and_abort(b, &format!("{unique}/delete")).await;
    hostile_keys(b).await;
    if b.naming() == fs_files_core::Naming::ById {
        strict_keys(b).await;
    }
}

/// Backends that name by id accept only plain keys; a backend that keeps real names accepts these as names.
async fn strict_keys(b: &dyn BlobBackend) {
    for k in [".hidden/x", "x y", "ü", ".fs-files/x"] {
        assert!(
            matches!(b.put(k, 0, b"x").await, Err(Error::InvalidKey(_))),
            "put {k:?}"
        );
        assert!(
            matches!(b.get(k, 0, 1).await, Err(Error::InvalidKey(_))),
            "get {k:?}"
        );
    }
}

fn capabilities(b: &dyn BlobBackend) {
    let c = b.capabilities();
    for id in ["blob.read", "blob.write", "blob.delete"] {
        assert!(c.is_supported(id), "{id}");
    }
}

async fn roundtrip(b: &dyn BlobBackend, key: &str) {
    assert_eq!(b.pending(key).await.unwrap(), 0);
    b.put(key, 0, b"hello ").await.unwrap();
    assert_eq!(b.pending(key).await.unwrap(), 6);
    assert!(b.stat(key).await.unwrap().is_none(), "pending is invisible");
    assert!(matches!(b.get(key, 0, 4).await, Err(Error::NotFound(_))));
    b.put(key, 6, b"world").await.unwrap();
    let info = b.commit(key).await.unwrap();
    assert_eq!(info.size, 11);
    assert_eq!(info.checksum, checksum(b"hello world"));
    assert_eq!(b.stat(key).await.unwrap().unwrap().size, 11);
    assert_eq!(b.get(key, 0, 5).await.unwrap(), b"hello");
    assert_eq!(b.get(key, 6, 100).await.unwrap(), b"world");
    assert!(b.get(key, 11, 5).await.unwrap().is_empty());
    assert!(b.get(key, 999, 5).await.unwrap().is_empty());
    assert_eq!(
        b.pending(key).await.unwrap(),
        0,
        "nothing pending after commit"
    );
    assert!(matches!(
        b.get("nobody/home", 0, 1).await,
        Err(Error::NotFound(_))
    ));
    assert!(b.stat("nobody/home").await.unwrap().is_none());
}

async fn wrong_offset(b: &dyn BlobBackend, key: &str) {
    assert!(matches!(
        b.put(key, 5, b"x").await,
        Err(Error::Offset {
            expected: 0,
            got: 5
        })
    ));
    b.put(key, 0, b"0123456789").await.unwrap();
    for bad in [0, 4, 11] {
        let err = b.put(key, bad, b"zz").await.unwrap_err();
        assert!(matches!(err, Error::Offset { expected: 10, .. }), "{err}");
    }
    assert_eq!(b.pending(key).await.unwrap(), 10, "no silent corruption");
    let info = b.commit(key).await.unwrap();
    assert_eq!(info.checksum, checksum(b"0123456789"));
    assert_eq!(b.get(key, 0, 99).await.unwrap(), b"0123456789");
    assert!(
        b.commit(&format!("{key}-never")).await.is_err(),
        "nothing pending"
    );
}

async fn empty_and_overwrite(b: &dyn BlobBackend, key: &str) {
    b.put(key, 0, &[]).await.unwrap();
    let info = b.commit(key).await.unwrap();
    assert_eq!((info.size, info.checksum), (0, checksum(b"")));
    b.put(key, 0, b"second").await.unwrap();
    if b.naming() == fs_files_core::Naming::ByName {
        // a backend that keeps real names never replaces: the second blob is a name clash and stays pending
        assert!(matches!(b.commit(key).await, Err(Error::Conflict(_))));
        assert_eq!(b.get(key, 0, 99).await.unwrap(), b"");
        assert_eq!(b.pending(key).await.unwrap(), 6);
        b.delete(key).await.unwrap();
        return;
    }
    b.commit(key).await.unwrap();
    assert_eq!(b.get(key, 0, 99).await.unwrap(), b"second");
}

async fn delete_and_abort(b: &dyn BlobBackend, key: &str) {
    b.put(key, 0, b"abc").await.unwrap();
    b.commit(key).await.unwrap();
    b.delete(key).await.unwrap();
    assert!(b.stat(key).await.unwrap().is_none());
    b.delete(key).await.unwrap(); // missing is fine
    let pending = format!("{key}-pending");
    b.put(&pending, 0, b"half").await.unwrap();
    b.delete(&pending).await.unwrap();
    assert_eq!(b.pending(&pending).await.unwrap(), 0, "abort cleans up");
    assert!(b.commit(&pending).await.is_err());
}

async fn hostile_keys(b: &dyn BlobBackend) {
    let keys = [
        "",
        "../x",
        "a/../b",
        "/etc/passwd",
        "a//b",
        "a/",
        "..",
        ".",
        "a\\b",
        "a\0b",
        "C:\\x",
        "a/./b",
        ".fs-files/x",
        &"k".repeat(600),
    ];
    for k in keys {
        assert!(
            matches!(b.put(k, 0, b"x").await, Err(Error::InvalidKey(_))),
            "put {k:?}"
        );
        assert!(
            matches!(b.get(k, 0, 1).await, Err(Error::InvalidKey(_))),
            "get {k:?}"
        );
        assert!(
            matches!(b.stat(k).await, Err(Error::InvalidKey(_))),
            "stat {k:?}"
        );
        assert!(
            matches!(b.commit(k).await, Err(Error::InvalidKey(_))),
            "commit {k:?}"
        );
        assert!(
            matches!(b.delete(k).await, Err(Error::InvalidKey(_))),
            "delete {k:?}"
        );
        assert!(
            matches!(b.pending(k).await, Err(Error::InvalidKey(_))),
            "pending {k:?}"
        );
    }
}
