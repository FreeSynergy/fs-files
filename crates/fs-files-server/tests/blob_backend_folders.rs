//! The provider interface `BlobBackend`, the folder half: make, scan, relocate, discard, inspect, sweep.

mod common;

use std::sync::Arc;

use common::backend::{CONFLICT, commit, path_spot, put, with_backend};
use common::{checksum, is};
use fs_files_core::LocalFolder;
use fs_interfaces::files_capnp::ScanKind;

#[tokio::test(flavor = "current_thread")]
async fn folders_scan_relocate_discard_inspect_and_sweep() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(LocalFolder::open(dir.path()).await.unwrap());
    with_backend(backend, |b| async move {
        let mut make = b.make_dir_request();
        make.get().set_path("A/B");
        make.get().set_parents(true);
        make.send().promise.await.unwrap();
        let mut again = b.make_dir_request();
        again.get().set_path("A/B");
        is(&again.send().promise.await.err().unwrap(), CONFLICT, 409);
        put(&b, "k", 0, b"data").await.unwrap();
        commit(&b, "k").await.unwrap();
        std::fs::write(dir.path().join("A/B/f.txt"), "one").unwrap();

        let mut scan = b.scan_request();
        scan.get().set_dir("A");
        let answer = scan.send().promise.await.unwrap();
        let items = answer.get().unwrap().get_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            (
                items.get(0).get_name().unwrap().to_str().unwrap(),
                items.get(0).get_kind().unwrap()
            ),
            ("B", ScanKind::Dir)
        );

        let mut relocate = b.relocate_request();
        path_spot(relocate.get().init_from(), "A/B/f.txt");
        path_spot(relocate.get().init_to(), "A/g.txt");
        relocate.send().promise.await.unwrap();
        assert!(dir.path().join("A/g.txt").is_file());
        let mut onto = b.relocate_request();
        path_spot(onto.get().init_from(), "k");
        path_spot(onto.get().init_to(), "A/g.txt");
        is(&onto.send().promise.await.err().unwrap(), CONFLICT, 409);
        let mut none = b.relocate_request();
        path_spot(none.get().init_from(), "nothing");
        path_spot(none.get().init_to(), "else");
        is(
            &none.send().promise.await.err().unwrap(),
            "https://freesynergy.net/problems/files/not-found",
            404,
        );

        let mut has = b.has_request();
        path_spot(has.get().init_spot(), "A/g.txt");
        assert!(has.send().promise.await.unwrap().get().unwrap().get_found());
        let mut inspect = b.inspect_request();
        path_spot(inspect.get().init_spot(), "A/g.txt");
        let answer = inspect.send().promise.await.unwrap();
        let info = answer.get().unwrap().get_info().unwrap();
        assert_eq!(
            (
                info.get_size(),
                info.get_checksum().unwrap().to_str().unwrap()
            ),
            (3, checksum(b"one").as_str())
        );

        let mut discard = b.discard_request();
        path_spot(discard.get().init_spot(), "A/g.txt");
        discard.send().promise.await.unwrap();
        assert!(!dir.path().join("A/g.txt").exists());
        let mut remove = b.remove_dir_request();
        remove.get().set_path("A/B");
        remove.send().promise.await.unwrap();
        assert!(!dir.path().join("A/B").exists());

        put(&b, "stale", 0, b"x").await.unwrap();
        let mut pending = b.has_request();
        pending.get().init_spot().set_pending("stale");
        assert!(
            pending
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_found()
        );
        let mut trash = b.has_request();
        trash.get().init_spot().set_trash("nothing-here");
        assert!(
            !trash
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_found()
        );
        let mut sweep = b.sweep_pending_request();
        sweep.get().init_keep(0);
        sweep.get().set_older_than_secs(0);
        let answer = sweep.send().promise.await.unwrap();
        assert_eq!(answer.get().unwrap().get_removed(), 1);
        let mut delete = b.delete_request();
        delete.get().set_key("k");
        delete.send().promise.await.unwrap();
        assert!(!dir.path().join("k").exists());
    })
    .await;
}
