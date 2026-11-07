//! The support report and the placeholders: what the program can do, said by the program itself.

mod common;

use common::{is, with_server};
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
use fs_interfaces::rpc::Problem;

#[tokio::test(flavor = "current_thread")]
async fn the_report_names_what_is_offered_and_why_the_rest_is_not() {
    with_server(|root, _| async move {
        let answer = root.support_report_request().send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(
            report.get_contract_version().unwrap().to_str().unwrap(),
            "files 0.2 (draft)"
        );
        assert_eq!(
            report.get_provider().unwrap().to_str().unwrap(),
            "fs-files-core"
        );
        let mut entries = std::collections::BTreeMap::new();
        for e in report.get_entries().unwrap() {
            let id = e.get_capability().unwrap().to_string().unwrap();
            let reason = e.get_reason().unwrap().to_string().unwrap();
            entries.insert(id, (e.get_supported(), reason));
        }
        for offered in [
            "file.upload",
            "file.download",
            "file.subscribe",
            "folder.rescan",
            "file.restoreTo",
            "file.emptyTrash",
            "folder.resolve",
            "file.pendingUploads",
            "backend.add",
        ] {
            assert!(entries[offered].0, "{offered}");
        }
        for (missing, why) in [
            (
                "share.create",
                "one specification for all programs, not written yet",
            ),
            ("file.versions", "not built yet"),
        ] {
            assert_eq!(
                (entries[missing].0, entries[missing].1.as_str()),
                (false, why),
                "{missing}"
            );
        }
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn sharing_answers_not_possible_with_its_capability_instead_of_an_empty_capability() {
    with_server(|root, _| async move {
        let error = root.sharing_request().send().promise.await.err().unwrap();
        is(&error, UNSUPPORTED_PROBLEM_TYPE, 501);
        let detail = Problem::from_error(&error).unwrap().detail;
        assert!(detail.contains("share.create"), "{detail}");
    })
    .await;
}
