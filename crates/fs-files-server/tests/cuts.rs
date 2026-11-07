//! What a provider returns can be longer than a message: the answer is cut and says so, the
//! connection stays. Nothing is cut without a trace except what `DESIGN.md` names.

mod common;

use common::stub::{Stub, entries, heavy_file};
use common::{caps, with_stub};
use fs_files_core::{RescanReport, UploadInfo};
use fs_objects::ObjectId;

#[tokio::test(flavor = "current_thread")]
async fn a_rescan_report_beyond_a_message_is_cut_and_the_cut_is_in_skipped() {
    let report = RescanReport {
        added: entries(30_000),
        updated: (0..10).map(heavy_file).collect(),
        missing: (0..10).map(heavy_file).collect(),
        skipped: (0..5).map(|i| format!("skipped {i}")).collect(),
    };
    let stub = Stub {
        rescan: report,
        ..Stub::default()
    };
    with_stub(stub, |root| async move {
        let c = caps(&root).await;
        let answer = c
            .folders
            .rescan_request()
            .send()
            .promise
            .await
            .expect("it arrives");
        let report = answer.get().unwrap().get_report().unwrap();
        let shown = report.get_added().unwrap().len() as usize;
        assert!(shown > 1000 && shown < 30_000, "{shown} entries shown");
        assert_eq!(
            report.get_updated().unwrap().len(),
            10,
            "the small lists are whole"
        );
        let first = common::read_entry(report.get_added().unwrap().get(0));
        assert!(first.0.starts_with("00000000"), "the first ones, in order");
        let skipped: Vec<String> = report
            .get_skipped()
            .unwrap()
            .iter()
            .map(|t| t.unwrap().to_string().unwrap())
            .collect();
        assert_eq!(
            &skipped[..5],
            [
                "skipped 0",
                "skipped 1",
                "skipped 2",
                "skipped 3",
                "skipped 4"
            ]
        );
        let note = format!(
            "... {} more added entries (they are taken over) are not listed here",
            30_000 - shown
        );
        assert_eq!(skipped[5..], [note], "the cut is named");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_trash_report_arrives_whole_with_the_count_of_the_purged() {
    let stub = Stub {
        failed: (0..1000).map(|_| ObjectId::new()).collect(),
        ..Stub::default()
    };
    let expected: Vec<_> = stub.failed.iter().map(|i| i.as_uuid()).collect();
    with_stub(stub, |root| async move {
        let c = caps(&root).await;
        let answer = c
            .folders
            .empty_trash_request()
            .send()
            .promise
            .await
            .unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_purged(), 3);
        let failed: Vec<_> = report
            .get_failed()
            .unwrap()
            .iter()
            .map(|u| fs_interfaces::rpc::read_uuid(u).unwrap())
            .collect();
        assert_eq!(failed, expected);
        assert_eq!(report.get_failed_missing(), 0, "all listed");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_trash_report_longer_than_a_message_lists_the_most_and_counts_the_rest() {
    let bound = 1_398_101; // 32 MiB at 24 bytes an id, as the contract says
    let stub = Stub {
        failed: (0..bound + 5).map(|_| ObjectId::new()).collect(),
        ..Stub::default()
    };
    with_stub(stub, |root| async move {
        let c = caps(&root).await;
        let answer = c
            .folders
            .empty_trash_request()
            .send()
            .promise
            .await
            .expect("it arrives");
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(report.get_failed().unwrap().len() as usize, bound);
        assert_eq!(report.get_failed_missing(), 5);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pending_uploads_come_oldest_first_in_pages_and_none_is_lost() {
    let uploads: Vec<UploadInfo> = (0..40_000)
        .map(|i| UploadInfo {
            id: format!("{i:08}"),
            folder: None,
            name: "n".repeat(255),
            media_type: "m".repeat(1000),
            received: i,
            started: chrono::DateTime::from_timestamp(1_700_000_000 + i64::try_from(i).unwrap(), 0)
                .unwrap(),
        })
        .collect();
    let stub = Stub {
        uploads,
        ..Stub::default()
    };
    with_stub(stub, |root| async move {
        let c = caps(&root).await;
        let (mut seen, mut cursor, mut pages) = (0usize, vec![], 0);
        loop {
            let mut call = c.transfer.pending_uploads_request();
            call.get().init_cursor().set_opaque(&cursor);
            let answer = call.send().promise.await.expect("a page arrives");
            let r = answer.get().unwrap();
            let items = r.get_items().unwrap();
            assert!(items.len() <= 100 && !items.is_empty());
            for item in items.iter() {
                let id = item.get_id().unwrap().to_str().unwrap();
                assert_eq!(id, format!("{seen:08}"), "oldest first, none skipped");
                seen += 1;
            }
            pages += 1;
            cursor = r.get_next().unwrap().get_opaque().unwrap().to_vec();
            if cursor.is_empty() {
                break;
            }
        }
        assert_eq!((seen, pages), (40_000, 400));
        let mut bad = c.transfer.pending_uploads_request();
        bad.get().init_cursor().set_opaque(b"bad");
        common::is(
            &bad.send().promise.await.err().unwrap(),
            fs_interfaces::rpc::INVALID_REQUEST,
            422,
        );
    })
    .await;
}
