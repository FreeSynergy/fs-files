//! The facts behind a problem are typed extensions, not only words in the detail: the id and kind of
//! the object, the name, the reasons, the sizes. Each problem type over the real socket.

mod common;

use common::stub::Stub;
use common::{caps, is, mkdir, put_file, set_id, start, text_file, unknown_id, with_server, with_stub};
use fs_interfaces::files_capnp::{
    BACKEND_FAILED_PROBLEM_TYPE, CHANGED_OUTSIDE_PROBLEM_TYPE, CONFLICT_PROBLEM_TYPE,
    ENCRYPTION_REQUIRED_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, INVALID_PROBLEM_TYPE,
    NAME_CONFLICT_PROBLEM_TYPE, NOT_FOUND_PROBLEM_TYPE, OnConflict, WRONG_OFFSET_PROBLEM_TYPE,
};
use fs_interfaces::rpc::{MAX_INTAKE_BYTES, Problem, TOO_LARGE};

/// The typed problem of `error`, of exactly this type.
fn typed(error: &capnp::Error, problem_type: &str) -> Problem {
    let p = Problem::from_error(error).expect("a typed problem");
    assert_eq!(p.problem_type, problem_type, "{}", p.detail);
    p
}

#[tokio::test(flavor = "current_thread")]
async fn not_found_names_the_file_folder_upload_or_backend_that_is_missing() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let unknown = unknown_id();

        let mut rename = c.folders.rename_file_request();
        set_id(rename.get().init_id(), unknown);
        rename.get().set_name("x.txt");
        let p = typed(&rename.send().promise.await.err().unwrap(), NOT_FOUND_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(unknown.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("file"));

        let error = mkdir(&c, Some(unknown), "inside").await.err().unwrap();
        let p = typed(&error, NOT_FOUND_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(unknown.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("folder"));

        let mut resume = c.transfer.resume_upload_request();
        resume.get().set_id(unknown.to_string().as_str());
        let p = typed(&resume.send().promise.await.err().unwrap(), NOT_FOUND_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(unknown.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("upload"));

        let mut remove = c.backends.remove_request();
        remove.get().set_id("nowhere");
        let p = typed(&remove.send().promise.await.err().unwrap(), NOT_FOUND_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some("nowhere"));
        assert_eq!(p.ext("kind"), Some("backend"));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_taken_name_is_named_and_a_refused_name_is_named_with_its_reason() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        text_file(&c, None, "a.txt", "x").await;
        let error = mkdir(&c, None, "a.txt").await.err().unwrap();
        let p = typed(&error, NAME_CONFLICT_PROBLEM_TYPE);
        assert_eq!(p.ext("name"), Some("a.txt"));
        assert!(p.detail.contains("a.txt"), "the detail is as it was");

        let again = put_file(&c, None, "a.txt", "", b"y", OnConflict::Refuse).await;
        let p = typed(&again.err().unwrap(), NAME_CONFLICT_PROBLEM_TYPE);
        assert_eq!(p.ext("name"), Some("a.txt"));

        let error = mkdir(&c, None, "a/b").await.err().unwrap();
        let p = typed(&error, INVALID_NAME_PROBLEM_TYPE);
        assert_eq!(p.ext("name"), Some("a/b"));
        assert_eq!(p.ext_all("issue").count(), 1, "one reason");
        assert!(p.detail.starts_with("invalid name: "), "{}", p.detail);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_changed_outside_is_named() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "a.txt", "original").await;
        std::fs::write(fx.root.join("a.txt"), "changed by someone else, longer").unwrap();
        let mut open = c.transfer.open_request();
        set_id(open.get().init_id(), file.id);
        let error = open.send().promise.await.err().unwrap();
        let p = typed(&error, CHANGED_OUTSIDE_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(file.id.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("file"));
        assert!(p.detail.starts_with("changed outside: "), "{}", p.detail);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_wrong_offset_says_where_the_upload_stands_and_where_the_chunk_started() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let (upload, _) = start(&c, None, "big.bin").await.unwrap();
        let mut first = upload.write_request();
        first.get().set_offset(0);
        first.get().set_data(&[1u8; 100]);
        first.send().promise.await.unwrap();
        let mut wrong = upload.write_request();
        wrong.get().set_offset(7);
        wrong.get().set_data(b"nope");
        let p = typed(&wrong.send().promise.await.err().unwrap(), WRONG_OFFSET_PROBLEM_TYPE);
        assert_eq!((p.ext_u64("expected"), p.ext_u64("got")), (Some(100), Some(7)));
        assert!(p.detail.contains("100"), "{}", p.detail);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_clash_with_the_state_an_invalid_backend_and_a_chunk_too_large_carry_their_facts() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "live.txt", "x").await;
        let mut purge = c.folders.purge_request();
        set_id(purge.get().init_id(), file.id);
        let p = typed(&purge.send().promise.await.err().unwrap(), CONFLICT_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(file.id.to_string().as_str()));
        assert_eq!(p.ext("kind"), Some("file"));

        let mut add = c.backends.add_request();
        add.get().set_config(r#"{"id":"odd","kind":"tape"}"#);
        let p = typed(&add.send().promise.await.err().unwrap(), INVALID_PROBLEM_TYPE);
        assert_eq!((p.ext("kind"), p.ext("id")), (Some("backend"), Some("odd")));
        assert_eq!(p.ext_all("issue").count(), 1);

        let (upload, _) = start(&c, None, "chunks.bin").await.unwrap();
        let mut over = upload.write_request();
        over.get().set_offset(0);
        over.get().set_data(&vec![1; MAX_INTAKE_BYTES + 1]);
        let p = typed(&over.send().promise.await.err().unwrap(), TOO_LARGE);
        assert_eq!(p.ext_u64("size"), Some(MAX_INTAKE_BYTES as u64 + 1));
        assert_eq!(p.ext_u64("limit"), Some(MAX_INTAKE_BYTES as u64));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_store_without_a_key_names_the_file_it_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = common::repo_on_disk(dir.path(), false).await;
    common::with_repo(repo, |root| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "tax.txt", "x").await;
        let mut call = c.folders.classify_request();
        set_id(call.get().init_id(), file.id);
        call.get().init_categories(1).set(0, "dpv:Financial");
        let error = call.send().promise.await.err().unwrap();
        let p = typed(&error, ENCRYPTION_REQUIRED_PROBLEM_TYPE);
        assert_eq!(p.ext("id"), Some(file.id.to_string().as_str()));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_failing_backend_is_named_as_the_backend() {
    let stub = Stub {
        fail_reads: true,
        ..Stub::default()
    };
    with_stub(stub, |root| async move {
        let c = caps(&root).await;
        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), unknown_id());
        let error = get.send().promise.await.err().unwrap();
        let p = typed(&error, BACKEND_FAILED_PROBLEM_TYPE);
        assert_eq!(p.ext("kind"), Some("backend"));
        assert_eq!(p.ext("id"), None, "no object is named");
    })
    .await;
}
