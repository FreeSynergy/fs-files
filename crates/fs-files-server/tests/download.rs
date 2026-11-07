//! Reading a file through its `Blob`: ranges, the end, the media type, a file changed outside.

mod common;

use common::{caps, download, set_id, text_file, with_server};
use fs_interfaces::files_capnp::CHANGED_OUTSIDE_PROBLEM_TYPE;

async fn blob(
    c: &common::Caps,
    id: uuid::Uuid,
) -> capnp::Result<fs_interfaces::common_capnp::blob::Client> {
    let mut open = c.transfer.open_request();
    set_id(open.get().init_id(), id);
    open.send().promise.await?.get()?.get_blob()
}

async fn read(
    blob: &fs_interfaces::common_capnp::blob::Client,
    offset: u64,
    length: u32,
) -> Vec<u8> {
    let mut call = blob.read_request();
    call.get().set_offset(offset);
    call.get().set_length(length);
    let answer = call.send().promise.await.unwrap();
    answer.get().unwrap().get_data().unwrap().to_vec()
}

#[tokio::test(flavor = "current_thread")]
async fn ranges_the_end_and_the_size_are_what_the_contract_says() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "digits.txt", "0123456789").await;
        let blob = blob(&c, file.id).await.unwrap();
        let size = blob.size_request().send().promise.await.unwrap();
        let size = size.get().unwrap();
        assert_eq!(size.get_bytes(), 10);
        assert_eq!(
            size.get_media_type().unwrap().to_str().unwrap(),
            "text/plain"
        );

        assert_eq!(read(&blob, 0, 4).await, b"0123");
        assert_eq!(
            read(&blob, 6, 100).await,
            b"6789",
            "fewer than asked at the end"
        );
        assert!(read(&blob, 10, 5).await.is_empty(), "empty is the end");
        assert!(read(&blob, 9_999, 5).await.is_empty());
        assert!(read(&blob, 0, 0).await.is_empty());
        assert_eq!(download(&c, file.id, 3).await.unwrap(), b"0123456789");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_changed_outside_is_an_error_and_never_wrong_data() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "a.txt", "original").await;
        std::fs::write(fx.root.join("a.txt"), "changed by someone else, longer").unwrap();
        let error = blob(&c, file.id).await.err().unwrap();
        common::is(&error, CHANGED_OUTSIDE_PROBLEM_TYPE, 409);
        std::fs::remove_file(fx.root.join("a.txt")).unwrap();
        let error = download(&c, file.id, 100).await.err().unwrap();
        common::is(&error, CHANGED_OUTSIDE_PROBLEM_TYPE, 409);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_trashed_or_unknown_file_cannot_be_opened() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "a.txt", "x").await;
        let mut trash = c.folders.trash_request();
        set_id(trash.get().init_id(), file.id);
        trash.send().promise.await.unwrap();
        let kind = |name: &str| format!("https://freesynergy.net/problems/files/{name}");
        let error = blob(&c, file.id).await.err().unwrap();
        common::is(&error, &kind("conflict"), 409);
        let error = blob(&c, common::unknown_id()).await.err().unwrap();
        common::is(&error, &kind("not-found"), 404);
    })
    .await;
}
