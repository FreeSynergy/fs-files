//! What does not fit one message, as a consumer sees it: refused or cut, and the connection stays.

mod common;

use common::{caps, is, list_all, repo_on_disk, set_id, text_file, with_repo, with_server};
use fs_interfaces::rpc::{INVALID_REQUEST, Problem, TOO_LARGE};

const MIB: usize = 1024 * 1024;

/// The ids exist nowhere: without the check at the door the core would answer "not found".
#[tokio::test(flavor = "current_thread")]
async fn every_text_is_refused_before_it_reaches_the_core() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let huge = "x".repeat(20 * MIB);
        let unknown = common::unknown_id();

        let mut media = c.transfer.start_upload_request();
        media.get().set_name("a");
        media.get().set_media_type(huge.as_str());
        is(
            &media.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut resume = c.transfer.resume_upload_request();
        resume.get().set_id(huge.as_str());
        is(
            &resume.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut classify = c.folders.classify_request();
        set_id(classify.get().init_id(), unknown);
        classify.get().init_categories(1).set(0, huge.as_str());
        is(
            &classify.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut many = c.folders.classify_request();
        set_id(many.get().init_id(), unknown);
        let mut list = many.get().init_categories(100);
        for i in 0..100 {
            list.set(i, "c".repeat(900).as_str());
        }
        let error = many.send().promise.await.err().unwrap();
        is(&error, INVALID_REQUEST, 422);
        assert!(
            Problem::from_error(&error)
                .unwrap()
                .detail
                .contains("together")
        );

        let mut remove = c.backends.remove_request();
        remove.get().set_id(huge.as_str());
        is(
            &remove.send().promise.await.err().unwrap(),
            INVALID_REQUEST,
            422,
        );

        let mut renamed = c.folders.rename_file_request();
        set_id(renamed.get().init_id(), unknown);
        renamed.get().set_name(huge.as_str());
        is(
            &renamed.send().promise.await.err().unwrap(),
            "https://freesynergy.net/problems/files/invalid-name",
            422,
        );
        assert!(list_all(&c, None).await.is_empty(), "still connected");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_chunk_above_the_limit_is_too_large_and_a_chunk_at_it_is_taken() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let (upload, _) = common::start(&c, None, "chunks.bin").await.unwrap();
        let limit = fs_interfaces::rpc::MAX_INTAKE_BYTES;
        let mut over = upload.write_request();
        over.get().set_offset(0);
        over.get().set_data(&vec![1; limit + 1]);
        let error = over.send().promise.await.err().unwrap();
        assert_eq!(Problem::from_error(&error).unwrap().problem_type, TOO_LARGE);
        assert_eq!(common::received(&upload).await, 0);
        let mut at = upload.write_request();
        at.get().set_offset(0);
        at.get().set_data(&vec![1; limit]);
        at.send().promise.await.unwrap();
        assert_eq!(common::received(&upload).await, limit as u64);
    })
    .await;
}

/// 100 files, each with the most sensitive categories the door lets in: a page is 100 entries, all of them heavy.
#[tokio::test(flavor = "current_thread")]
async fn a_page_of_the_heaviest_entries_still_fits_and_nothing_is_lost() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repo_on_disk(dir.path(), true).await;
    with_repo(repo, |root| async move {
        let c = caps(&root).await;
        let heavy: Vec<String> = (0..64)
            .map(|i| format!("{i:02}{}", "c".repeat(1022)))
            .collect();
        for n in 0..100 {
            let file = text_file(&c, None, &format!("f-{n:03}"), "x").await;
            let mut classify = c.folders.classify_request();
            set_id(classify.get().init_id(), file.id);
            let mut list = classify.get().init_categories(64);
            for (i, cat) in heavy.iter().enumerate() {
                list.set(i as u32, cat.as_str());
            }
            classify.send().promise.await.unwrap();
        }
        let mut first = c.folders.list_request();
        first.get().init_folder();
        let answer = first.send().promise.await.expect("a heavy page arrives");
        let page = answer.get().unwrap().get_page().unwrap();
        assert_eq!(page.get_items().unwrap().len(), 100);
        let entry = common::read_entry(page.get_items().unwrap().get(99));
        assert_eq!(entry.0, "f-099");
        let sensitive = match page.get_items().unwrap().get(0).which().unwrap() {
            fs_interfaces::files_capnp::entry::Which::File(f) => {
                common::read_file(f.unwrap()).envelope.sensitive
            }
            fs_interfaces::files_capnp::entry::Which::Folder(_) => unreachable!(),
        };
        assert_eq!(sensitive, heavy);
        assert_eq!(list_all(&c, None).await.len(), 100);
    })
    .await;
}
