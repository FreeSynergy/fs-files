//! The trash over the wire: trash, restore, restore elsewhere, purge, empty; nothing is deleted before purge.

mod common;

use common::{caps, is, list_all, mkdir, names, read_file, set_id, text_file, with_server};
use fs_interfaces::files_capnp::{NAME_CONFLICT_PROBLEM_TYPE, OnConflict};
use fs_interfaces::rpc::write_uuid;

const CONFLICT: &str = "https://freesynergy.net/problems/files/conflict";

async fn trash(c: &common::Caps, id: uuid::Uuid) {
    let mut call = c.folders.trash_request();
    set_id(call.get().init_id(), id);
    call.send().promise.await.unwrap();
}

async fn found_trashed(c: &common::Caps) -> Vec<String> {
    let mut find = c.search.find_request();
    find.get().init_filter().set_trashed(true);
    let answer = find.send().promise.await.unwrap();
    let page = answer.get().unwrap().get_page().unwrap();
    page.get_items()
        .unwrap()
        .iter()
        .map(|e| common::read_entry(e).0)
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_trashed_file_is_hidden_found_in_the_trash_and_restored() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "a.txt", "keep me").await;
        trash(&c, file.id).await;
        assert!(list_all(&c, None).await.is_empty());
        assert!(!fx.root.join("a.txt").exists());
        assert_eq!(found_trashed(&c).await, ["a.txt"]);
        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), file.id);
        let answer = get.send().promise.await.unwrap();
        assert!(read_file(answer.get().unwrap().get_file().unwrap()).trashed);

        let mut restore = c.folders.restore_request();
        set_id(restore.get().init_id(), file.id);
        let answer = restore.send().promise.await.unwrap();
        let back = read_file(answer.get().unwrap().get_file().unwrap());
        assert!(!back.trashed);
        assert_eq!(std::fs::read(fx.root.join("a.txt")).unwrap(), b"keep me");
        assert_eq!(names(&list_all(&c, None).await), ["a.txt"]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn restore_into_a_taken_name_is_refused_keep_both_or_another_folder_helps() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let old = text_file(&c, None, "a.txt", "old").await;
        trash(&c, old.id).await;
        text_file(&c, None, "a.txt", "new").await;

        let mut plain = c.folders.restore_request();
        set_id(plain.get().init_id(), old.id);
        is(
            &plain.send().promise.await.err().unwrap(),
            NAME_CONFLICT_PROBLEM_TYPE,
            409,
        );
        assert_eq!(
            found_trashed(&c).await,
            ["a.txt"],
            "the bytes stay in the trash"
        );

        let dir = mkdir(&c, None, "Back").await.unwrap();
        let mut to = c.folders.restore_to_request();
        set_id(to.get().init_id(), old.id);
        write_uuid(to.get().init_folder(), Some(dir));
        let answer = to.send().promise.await.unwrap();
        let file = read_file(answer.get().unwrap().get_file().unwrap());
        assert_eq!(file.folder, Some(dir));
        assert_eq!(std::fs::read(fx.root.join("Back/a.txt")).unwrap(), b"old");
        assert_eq!(std::fs::read(fx.root.join("a.txt")).unwrap(), b"new");

        trash(&c, file.id).await;
        text_file(&c, Some(dir), "a.txt", "taken again").await;
        let mut both = c.folders.restore_request();
        set_id(both.get().init_id(), file.id);
        both.get().set_on_conflict(OnConflict::KeepBoth);
        let answer = both.send().promise.await.unwrap();
        let kept = read_file(answer.get().unwrap().get_file().unwrap());
        assert_eq!(kept.name, "a (2).txt");
        assert_eq!(kept.folder, Some(dir), "restored to its original folder");
        assert_eq!(
            std::fs::read(fx.root.join("Back/a (2).txt")).unwrap(),
            b"old"
        );
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn purge_needs_the_trash_and_is_for_good_and_empty_trash_reports() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let live = text_file(&c, None, "live.txt", "x").await;
        let mut early = c.folders.purge_request();
        set_id(early.get().init_id(), live.id);
        is(&early.send().promise.await.err().unwrap(), CONFLICT, 409);

        let (a, b) = (
            text_file(&c, None, "a.txt", "aa").await,
            text_file(&c, None, "b.txt", "bb").await,
        );
        trash(&c, a.id).await;
        let mut purge = c.folders.purge_request();
        set_id(purge.get().init_id(), a.id);
        purge.send().promise.await.unwrap();
        let mut gone = c.folders.get_request();
        set_id(gone.get().init_id(), a.id);
        assert!(
            !gone
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_found()
        );

        trash(&c, b.id).await;
        let answer = c
            .folders
            .empty_trash_request()
            .send()
            .promise
            .await
            .unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_purged(), 1);
        assert_eq!(report.get_failed().unwrap().len(), 0);
        assert!(found_trashed(&c).await.is_empty());
        assert_eq!(names(&list_all(&c, None).await), ["live.txt"]);
        assert!(fx.root.join("live.txt").is_file());
    })
    .await;
}
