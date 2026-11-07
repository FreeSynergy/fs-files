//! The provider interface `BlobBackend` in front of a backend that can only read: every method answers,
//! and what a backend cannot do says so, in the answer and in its report.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::backend::{commit, put, with_backend};
use common::is;
use fs_files_core::{BlobBackend, Error, Result};
use fs_interfaces::Capabilities;
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;

struct ReadOnly;

#[async_trait]
impl BlobBackend for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("read-only");
        c.unsupported("blob.write", "this backend cannot be written");
        c
    }
    fn kind(&self) -> &'static str {
        "read-only"
    }
    async fn get(&self, key: &str, _: u64, _: u32) -> Result<Vec<u8>> {
        Err(Error::NotFound(key.into()))
    }
    async fn stat(&self, _: &str) -> Result<Option<fs_files_core::BlobInfo>> {
        Ok(None)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_backend_that_cannot_do_a_method_answers_not_possible_and_reports_it() {
    with_backend(Arc::new(ReadOnly), |b| async move {
        assert!(
            !b.naming_request()
                .send()
                .promise
                .await
                .unwrap()
                .get()
                .unwrap()
                .get_by_name()
        );
        for error in [
            put(&b, "k", 0, b"x").await.err().unwrap(),
            commit(&b, "k").await.err().unwrap(),
        ] {
            is(&error, UNSUPPORTED_PROBLEM_TYPE, 501);
        }
        let mut publish = b.publish_request();
        publish.get().set_pending("p");
        publish.get().set_key("k");
        is(
            &publish.send().promise.await.err().unwrap(),
            UNSUPPORTED_PROBLEM_TYPE,
            501,
        );
        let mut scan = b.scan_request();
        scan.get().set_dir("");
        is(
            &scan.send().promise.await.err().unwrap(),
            UNSUPPORTED_PROBLEM_TYPE,
            501,
        );

        let answer = b.support_report_request().send().promise.await.unwrap();
        let report = answer.get().unwrap().get_report().unwrap();
        assert_eq!(
            report.get_provider().unwrap().to_str().unwrap(),
            "read-only"
        );
        let entry = report.get_entries().unwrap().get(0);
        assert_eq!(
            (
                entry.get_capability().unwrap().to_str().unwrap(),
                entry.get_supported()
            ),
            ("blob.write", false)
        );
    })
    .await;
}
