//! The provider interface `BlobBackend` over a real socket, in front of the local folder: bytes go
//! in pending, are published under real names, and nothing is ever replaced.

mod common;

use std::sync::Arc;

use common::backend::{CONFLICT, put, with_backend};
use common::{checksum, is};
use fs_files_core::LocalFolder;

#[tokio::test(flavor = "current_thread")]
async fn bytes_go_in_pending_and_are_published_under_real_names_and_never_replace() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(LocalFolder::open(dir.path()).await.unwrap());
    with_backend(backend, |b| async move {
        let naming = b.naming_request().send().promise.await.unwrap();
        assert!(naming.get().unwrap().get_by_name());

        put(&b, "up-1", 0, b"hello ").await.unwrap();
        let mut held = b.pending_request();
        held.get().set_key("up-1");
        assert_eq!(
            held.send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_bytes(),
            6
        );
        let wrong = put(&b, "up-1", 2, b"x").await.err().unwrap();
        is(
            &wrong,
            "https://freesynergy.net/problems/files/wrong-offset",
            409,
        );
        put(&b, "up-1", 6, b"world").await.unwrap();

        let mut publish = b.publish_request();
        publish.get().set_pending("up-1");
        publish.get().set_key("Docs/hello.txt");
        let answer = publish.send().promise.await.unwrap();
        let info = answer.get().unwrap().get_info().unwrap();
        assert_eq!(
            (
                info.get_size(),
                info.get_checksum().unwrap().to_str().unwrap()
            ),
            (11, checksum(b"hello world").as_str())
        );
        assert_eq!(
            std::fs::read(dir.path().join("Docs/hello.txt")).unwrap(),
            b"hello world"
        );

        put(&b, "up-2", 0, b"other").await.unwrap();
        let mut again = b.publish_request();
        again.get().set_pending("up-2");
        again.get().set_key("Docs/hello.txt");
        is(&again.send().promise.await.err().unwrap(), CONFLICT, 409);
        assert_eq!(
            std::fs::read(dir.path().join("Docs/hello.txt")).unwrap(),
            b"hello world"
        );

        let mut stat = b.stat_request();
        stat.get().set_key("Docs/hello.txt");
        let answer = stat.send().promise.await.unwrap();
        assert!(answer.get().unwrap().get_found());
        assert_eq!(answer.get().unwrap().get_info().unwrap().get_size(), 11);
        let mut get = b.get_request();
        get.get().set_key("Docs/hello.txt");
        get.get().set_offset(6);
        get.get().set_length(u32::MAX);
        let answer = get.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_data().unwrap(), b"world");
        let mut missing = b.stat_request();
        missing.get().set_key("Docs/none.txt");
        assert!(
            !missing
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_found()
        );
    })
    .await;
}
