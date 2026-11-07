//! What a provider returns can be longer than a message: the answer is cut and says so, the
//! connection stays. Nothing is cut without a trace except what `DESIGN.md` names.

mod common;

use common::stub::{Stub, entries, heavy_file};
use common::{caps, with_stub};
use fs_files_core::{RescanReport, UploadInfo};
use fs_interfaces::rpc::MAX_PAGE_BYTES;
use fs_store::ObjectId;

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
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pending_uploads_beyond_a_message_come_oldest_first_as_many_as_fit() {
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
        let answer = c
            .transfer
            .pending_uploads_request()
            .send()
            .promise
            .await
            .expect("it arrives");
        let items = answer.get().unwrap().get_items().unwrap();
        let shown = items.len() as usize;
        assert!(shown > 1000 && shown < 40_000, "{shown} uploads shown");
        assert!(shown * 1500 < MAX_PAGE_BYTES * 2);
        for (i, item) in items.iter().enumerate() {
            assert_eq!(
                item.get_id().unwrap().to_str().unwrap(),
                format!("{i:08}"),
                "oldest first, none skipped"
            );
        }
    })
    .await;
}
