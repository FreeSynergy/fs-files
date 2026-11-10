//! `rescan` over the wire: what changed outside the program is reported, nothing is deleted.

mod common;

use common::{caps, mkdir, read_entry, read_file, set_id, text_file, with_server};
use fs_interfaces::rpc::write_uuid;

fn texts(list: capnp::text_list::Reader<'_>) -> Vec<String> {
    list.iter()
        .map(|t| t.unwrap().to_string().unwrap())
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn new_changed_and_vanished_files_are_reported_and_no_record_is_deleted() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let dir = mkdir(&c, None, "Docs").await.unwrap();
        let changed = text_file(&c, Some(dir), "changed.txt", "one").await;
        let vanished = text_file(&c, Some(dir), "vanished.txt", "bye").await;
        let kept = text_file(&c, Some(dir), "kept.txt", "same").await;

        std::fs::write(fx.root.join("Docs/found.txt"), "new on the disk").unwrap();
        std::fs::create_dir(fx.root.join("Docs/Sub")).unwrap();
        std::fs::write(fx.root.join("Docs/changed.txt"), "one, and more").unwrap();
        std::fs::remove_file(fx.root.join("Docs/vanished.txt")).unwrap();
        let target = fx.root.join("target.txt");
        std::fs::write(&target, "elsewhere").unwrap();
        common::links::link_file(&target, &fx.root.join("Docs/link"));

        let mut call = c.folders.rescan_request();
        write_uuid(call.get().init_folder(), Some(dir));
        let answer = call.send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();

        let mut added: Vec<_> = report.get_added().unwrap().iter().map(read_entry).collect();
        added.sort();
        let named: Vec<_> = added.iter().map(|e| (e.0.as_str(), e.1)).collect();
        assert_eq!(named, [("Sub", true), ("found.txt", false)]);
        let updated: Vec<_> = report
            .get_updated()
            .unwrap()
            .iter()
            .map(read_file)
            .collect();
        assert_eq!(updated.len(), 1);
        assert_eq!((updated[0].id, updated[0].size), (changed.id, 13));
        assert_ne!(updated[0].checksum, changed.checksum);
        let missing: Vec<_> = report
            .get_missing()
            .unwrap()
            .iter()
            .map(read_file)
            .collect();
        assert_eq!(
            missing.iter().map(|f| f.id).collect::<Vec<_>>(),
            [vanished.id]
        );
        let skipped = texts(report.get_skipped().unwrap());
        assert!(
            skipped.iter().any(|s| s.starts_with("Docs/link")),
            "{skipped:?}"
        );

        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), vanished.id);
        let answer = get.send().promise.await.unwrap();
        assert!(
            answer.get().unwrap().get_found(),
            "the record of a vanished file is kept"
        );
        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), kept.id);
        let answer = get.send().promise.await.unwrap();
        assert_eq!(read_file(answer.get().unwrap().get_file().unwrap()), kept);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_unset_folder_means_the_top_and_a_second_rescan_finds_nothing_new() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        std::fs::write(fx.root.join("top.txt"), "x").unwrap();
        for expected in [1, 0] {
            let answer = c.folders.rescan_request().send().promise.await.unwrap();
            let report = answer.get().unwrap().get_report().unwrap();
            assert_eq!(report.get_added().unwrap().len(), expected);
        }
    })
    .await;
}
