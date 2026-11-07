//! A taken name and a refused name, as the contract names them: exact problem types; nothing is replaced.

mod common;

use common::{caps, is, list_all, mkdir, names, put_file, set_id, text_file, with_server};
use fs_interfaces::files_capnp::{
    INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE, OnConflict,
};
use fs_interfaces::rpc::write_uuid;

#[tokio::test(flavor = "current_thread")]
async fn a_taken_name_is_refused_by_default_and_changes_nothing() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let first = text_file(&c, None, "a.txt", "first").await;
        let second = text_file(&c, None, "b.txt", "second").await;

        let mut rename = c.folders.rename_file_request();
        set_id(rename.get().init_id(), second.id);
        rename.get().set_name("a.txt");
        let error = rename.send().promise.await.err().unwrap();
        is(&error, NAME_CONFLICT_PROBLEM_TYPE, 409);

        let again = put_file(&c, None, "a.txt", "", b"third", OnConflict::Refuse).await;
        is(&again.err().unwrap(), NAME_CONFLICT_PROBLEM_TYPE, 409);

        let mut mkdir_again = c.folders.create_folder_request();
        mkdir_again.get().init_parent();
        mkdir_again.get().set_name("a.txt");
        is(
            &mkdir_again.send().promise.await.err().unwrap(),
            NAME_CONFLICT_PROBLEM_TYPE,
            409,
        );

        assert_eq!(std::fs::read(fx.root.join("a.txt")).unwrap(), b"first");
        assert_eq!(std::fs::read(fx.root.join("b.txt")).unwrap(), b"second");
        let all = list_all(&c, None).await;
        assert_eq!(names(&all), ["a.txt", "b.txt"]);
        assert_eq!((all[0].2, all[1].2), (first.id, second.id));
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn keep_both_gives_the_incoming_entry_a_free_name() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let a = text_file(&c, None, "a.txt", "first").await;
        let kept = put_file(&c, None, "a.txt", "", b"second", OnConflict::KeepBoth).await;
        assert_eq!(kept.unwrap().name, "a (2).txt");

        let mut copy = c.folders.copy_file_request();
        set_id(copy.get().init_id(), a.id);
        copy.get().init_folder();
        copy.get().set_on_conflict(OnConflict::KeepBoth);
        let answer = copy.send().promise.await.unwrap();
        let name = common::read_file(answer.get().unwrap().get_file().unwrap()).name;
        assert_eq!(name, "a (copy).txt");

        let other = mkdir(&c, None, "Other").await.unwrap();
        text_file(&c, Some(other), "a.txt", "in other").await;
        let mut moved = c.folders.move_file_request();
        set_id(moved.get().init_id(), a.id);
        write_uuid(moved.get().init_folder(), Some(other));
        let error = moved.send().promise.await.err().unwrap();
        is(&error, NAME_CONFLICT_PROBLEM_TYPE, 409);

        assert_eq!(std::fs::read(fx.root.join("a.txt")).unwrap(), b"first");
        assert_eq!(std::fs::read(fx.root.join("a (2).txt")).unwrap(), b"second");
        assert_eq!(
            std::fs::read(fx.root.join("Other/a.txt")).unwrap(),
            b"in other"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn names_the_contract_refuses_are_invalid_names() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let long = "x".repeat(256);
        let huge = "x".repeat(20 * 1024 * 1024);
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "tab\there",
            ".fs-files",
            long.as_str(),
            huge.as_str(),
        ] {
            let mut folder = c.folders.create_folder_request();
            folder.get().init_parent();
            folder.get().set_name(bad);
            let error = folder.send().promise.await.err().unwrap();
            is(&error, INVALID_NAME_PROBLEM_TYPE, 422);
            let detail = fs_interfaces::rpc::Problem::from_error(&error)
                .unwrap()
                .detail;
            assert!(
                detail.len() < 4096,
                "an error text must stay short: {} bytes",
                detail.len()
            );

            let file = put_file(&c, None, bad, "", b"x", OnConflict::Refuse).await;
            is(&file.err().unwrap(), INVALID_NAME_PROBLEM_TYPE, 422);
        }
        let ok = "y".repeat(255);
        text_file(&c, None, &ok, "fits").await;
        assert_eq!(
            list_all(&c, None).await.len(),
            1,
            "only the good name was made, and we are still connected"
        );
    })
    .await;
}
