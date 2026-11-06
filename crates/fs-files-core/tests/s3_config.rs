//! S3 configuration: credentials stay secret, bad input is refused, an unreachable server is found at `backend.add`.
#![cfg(feature = "s3")]

mod common;

use common::env;
use fs_files_core::{Backends, Error, S3Config};
use serde_json::json;

const SECRET: &str = "super-secret-key-1234";

fn config(extra: serde_json::Value) -> serde_json::Value {
    let mut c = json!({"bucket": "b", "access_key": "AKIA-ID", "secret_key": SECRET});
    c.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    c
}

#[test]
fn debug_output_hides_the_credentials() {
    let c = S3Config::from_json(&config(json!({"endpoint": "http://127.0.0.1:9000"}))).unwrap();
    let shown = format!("{c:?}");
    assert!(
        !shown.contains(SECRET) && !shown.contains("AKIA-ID"),
        "{shown}"
    );
    assert!(shown.contains("127.0.0.1"));
}

#[test]
fn bad_configurations_are_refused_without_echoing_them() {
    for bad in [
        json!({"bucket": 5, "access_key": "x", "secret_key": SECRET}),
        json!({"bucket": "b", "access_key": "", "secret_key": SECRET}),
        config(json!({"part_size_mib": 1})),
        json!({"bucket": "b", "access_key": "x", "secret_key": ["s", SECRET]}),
    ] {
        let err = S3Config::from_json(&bad).unwrap_err().to_string();
        assert!(!err.contains(SECRET), "{err}");
    }
}

#[tokio::test]
async fn an_unreachable_server_is_found_before_the_swap() {
    let env = env().await;
    let before = env.repo.list_backends().await.unwrap();
    let cfg = config(
        json!({"id": "cloud", "kind": "s3", "endpoint": "http://127.0.0.1:1", "active": true}),
    );
    let err = env.repo.add_backend(&cfg.to_string()).await.unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");
    assert!(!err.to_string().contains(SECRET));
    assert_eq!(
        env.repo.list_backends().await.unwrap(),
        before,
        "nothing changed"
    );
}
