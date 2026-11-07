//! Uploads over the wire: chunks, position, wrong offset, pipelined chunks, resume, abort.

mod common;

use common::{caps, checksum, is, list_all, read_file, received, start, with_server};
use fs_interfaces::rpc::{Problem, TOO_LARGE};

const WRONG_OFFSET: &str = "https://freesynergy.net/problems/files/wrong-offset";

#[tokio::test(flavor = "current_thread")]
async fn chunks_make_one_file_and_the_position_is_what_was_written() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let (upload, _) = start(&c, None, "big.bin").await.unwrap();
        let (one, two) = (vec![1u8; 100_000], vec![2u8; 50_000]);
        let mut write = upload.write_request();
        write.get().set_offset(0);
        write.get().set_data(&one);
        write.send().promise.await.unwrap();
        assert_eq!(received(&upload).await, 100_000);

        let mut wrong = upload.write_request();
        wrong.get().set_offset(7);
        wrong.get().set_data(b"nope");
        is(
            &wrong.send().promise.await.err().unwrap(),
            WRONG_OFFSET,
            409,
        );
        assert_eq!(received(&upload).await, 100_000, "nothing was written");

        let mut write = upload.write_request();
        write.get().set_offset(100_000);
        write.get().set_data(&two);
        write.send().promise.await.unwrap();
        assert!(
            list_all(&c, None).await.is_empty(),
            "not a file before finish"
        );

        let done = upload.finish_request().send().promise.await.unwrap();
        let file = read_file(done.get().unwrap().get_file().unwrap());
        let all = [one, two].concat();
        assert_eq!((file.size, file.checksum), (150_000, checksum(&all)));
        assert_eq!(std::fs::read(fx.root.join("big.bin")).unwrap(), all);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn chunks_sent_without_waiting_for_each_answer_arrive_in_order() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let (upload, _) = start(&c, None, "piped.bin").await.unwrap();
        let (mut calls, mut all) = (vec![], vec![]);
        for i in 0u8..8 {
            let chunk = vec![i; 4096];
            let mut write = upload.write_request();
            write.get().set_offset(all.len() as u64);
            write.get().set_data(&chunk);
            calls.push(write.send().promise);
            all.extend(chunk);
        }
        // The finish goes out behind the chunks, before any answer came.
        let finishing = upload.finish_request().send().promise;
        for call in calls {
            call.await.unwrap();
        }
        let done = finishing.await.unwrap();
        let file = read_file(done.get().unwrap().get_file().unwrap());
        assert_eq!(file.checksum, checksum(&all));
        assert_eq!(std::fs::read(fx.root.join("piped.bin")).unwrap(), all);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_unfinished_upload_is_listed_resumed_on_a_new_call_and_aborted() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let dir = common::mkdir(&c, None, "In").await.unwrap();
        let (first, id) = start(&c, Some(dir), "resume.bin").await.unwrap();
        let mut write = first.write_request();
        write.get().set_offset(0);
        write.get().set_data(b"abcd");
        write.send().promise.await.unwrap();
        drop(first);

        let pending = c
            .transfer
            .pending_uploads_request()
            .send()
            .promise
            .await
            .unwrap();
        let items = pending.get().unwrap().get_items().unwrap();
        assert_eq!(items.len(), 1);
        let info = items.get(0);
        assert_eq!(info.get_id().unwrap().to_str().unwrap(), id);
        assert_eq!(info.get_name().unwrap().to_str().unwrap(), "resume.bin");
        assert_eq!(
            (
                info.get_received(),
                info.get_started().unwrap().get_unix_millis() > 0
            ),
            (4, true)
        );
        assert_eq!(common::read_uuid_of(info.get_folder().unwrap()), Some(dir));

        let mut resume = c.transfer.resume_upload_request();
        resume.get().set_id(id.as_str());
        let answer = resume.send().promise.await.unwrap();
        let upload = answer.get().unwrap().get_upload().unwrap();
        assert_eq!(received(&upload).await, 4);
        let mut write = upload.write_request();
        write.get().set_offset(4);
        write.get().set_data(b"efgh");
        write.send().promise.await.unwrap();
        let done = upload.finish_request().send().promise.await.unwrap();
        assert_eq!(
            read_file(done.get().unwrap().get_file().unwrap()).checksum,
            checksum(b"abcdefgh")
        );

        let (second, _) = start(&c, None, "dropped.bin").await.unwrap();
        second.abort_request().send().promise.await.unwrap();
        let pending = c
            .transfer
            .pending_uploads_request()
            .send()
            .promise
            .await
            .unwrap();
        assert_eq!(pending.get().unwrap().get_items().unwrap().len(), 0);
        let mut gone = c.transfer.resume_upload_request();
        gone.get().set_id("no-such-upload");
        is(
            &gone.send().promise.await.err().unwrap(),
            "https://freesynergy.net/problems/files/not-found",
            404,
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_media_type_is_detected_when_none_is_given_and_a_huge_chunk_is_refused_whole() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let file = common::put_file(
            &c,
            None,
            "p.png",
            "",
            png,
            fs_interfaces::files_capnp::OnConflict::Refuse,
        )
        .await
        .unwrap();
        assert_eq!(file.media_type, "image/png");
        assert_eq!(file.envelope.object_type, "Image");

        let (upload, _) = start(&c, None, "huge.bin").await.unwrap();
        let mut write = upload.write_request();
        write.get().set_offset(0);
        write.get().set_data(&vec![0u8; 17 * 1024 * 1024]);
        let error = write.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&error).unwrap().problem_type, TOO_LARGE);
        assert_eq!(
            received(&upload).await,
            0,
            "and the connection is still there"
        );
        let taken = start(&c, None, "p.png").await.err().unwrap();
        is(
            &taken,
            fs_interfaces::files_capnp::NAME_CONFLICT_PROBLEM_TYPE,
            409,
        );
    })
    .await;
}
