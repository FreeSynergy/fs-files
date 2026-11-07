//! Bytes that are no UTF-8 in a text of a request are a bad request, typed, and the connection stays.

mod common;

use common::backend::with_backend;
use common::{caps, is, list_all, with_server};
use fs_files_core::LocalFolder;
use fs_interfaces::rpc::INVALID_REQUEST;
use std::sync::Arc;

const BAD: [u8; 3] = [b'a', 0xff, 0xfe];

fn bad_text(text: capnp::text::Builder<'_>) {
    text.as_bytes_mut().copy_from_slice(&BAD);
}

#[tokio::test(flavor = "current_thread")]
async fn every_kind_of_text_refuses_bytes_that_are_no_utf8() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let mut folder = c.folders.create_folder_request();
        folder.get().init_parent();
        bad_text(folder.get().init_name(3));
        is(
            &folder.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut upload = c.transfer.start_upload_request();
        upload.get().set_name("ok");
        bad_text(upload.get().init_media_type(3));
        is(
            &upload.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut find = c.search.find_request();
        bad_text(find.get().init_filter().init_name(3));
        is(
            &find.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut add = c.backends.add_request();
        bad_text(add.get().init_config(3));
        is(
            &add.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        assert!(list_all(&c, None).await.is_empty(), "still connected");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_key_or_a_spot_of_a_backend_refuses_them_too() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(LocalFolder::open(dir.path()).await.unwrap());
    with_backend(backend, |b| async move {
        let mut stat = b.stat_request();
        bad_text(stat.get().init_key(3));
        is(
            &stat.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );
        let mut has = b.has_request();
        bad_text(has.get().init_spot().init_trash(3));
        is(
            &has.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );
    })
    .await;
}
