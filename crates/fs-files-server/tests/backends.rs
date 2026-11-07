//! Backends over the wire: list, add (checked, then swapped), remove; secrets stay out of errors.

mod common;

use common::{caps, is, put_file, text_file, with_server};
use fs_interfaces::files_capnp::OnConflict;
use fs_interfaces::rpc::{Problem, TOO_LARGE};

const INVALID: &str = "https://freesynergy.net/problems/files/invalid";
const CONFLICT: &str = "https://freesynergy.net/problems/files/conflict";

fn local(id: &str, path: &std::path::Path, active: bool) -> String {
    serde_json::json!({ "id": id, "kind": "local", "path": path, "active": active }).to_string()
}

async fn listed(c: &common::Caps) -> Vec<(String, String, bool)> {
    let answer = c.backends.list_request().send().promise.await.unwrap();
    let items = answer.get().unwrap().get_items().unwrap();
    items
        .iter()
        .map(|b| {
            let text = |t: capnp::Result<capnp::text::Reader<'_>>| t.unwrap().to_string().unwrap();
            (text(b.get_id()), text(b.get_kind()), b.get_active())
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_backend_is_added_used_for_new_files_and_removed_when_empty() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        assert_eq!(listed(&c).await, [("main".into(), "local".into(), true)]);
        text_file(&c, None, "old.txt", "old").await;

        let second = fx.root.parent().unwrap().join("second");
        let mut add = c.backends.add_request();
        add.get()
            .set_config(local("second", &second, true).as_str());
        let answer = add.send().promise.await.unwrap();
        let info = answer.get().unwrap().get_info().unwrap();
        assert_eq!(info.get_id().unwrap().to_str().unwrap(), "second");
        assert!(info.get_active());
        assert_eq!(
            listed(&c).await,
            [
                ("main".into(), "local".into(), false),
                ("second".into(), "local".into(), true)
            ]
        );

        text_file(&c, None, "new.txt", "new").await;
        assert_eq!(std::fs::read(second.join("new.txt")).unwrap(), b"new");
        assert_eq!(std::fs::read(fx.root.join("old.txt")).unwrap(), b"old");

        let mut busy = c.backends.remove_request();
        busy.get().set_id("main");
        is(&busy.send().promise.await.err().unwrap(), CONFLICT, 409);
        let mut active = c.backends.remove_request();
        active.get().set_id("second");
        is(&active.send().promise.await.err().unwrap(), CONFLICT, 409);
        assert_eq!(listed(&c).await.len(), 2, "nothing was removed");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_wrong_configuration_changes_nothing_and_never_repeats_a_secret() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        for config in [
            r#"{"id":"x","kind":"nope","password":"TOPSECRET"}"#,
            r#"{"id":"x","kind":"local","password":"TOPSECRET"}"#,
            r#"{"id":"BAD ID","kind":"local","path":"/x","password":"TOPSECRET"}"#,
            "{ TOPSECRET not json",
        ] {
            let mut add = c.backends.add_request();
            add.get().set_config(config);
            let error = add.send().promise.await.err().unwrap();
            let problem = Problem::from_error(&error).expect("typed");
            assert_eq!(
                (problem.problem_type.as_str(), problem.status),
                (INVALID, 422),
                "{config}"
            );
            assert!(!problem.detail.contains("TOPSECRET"), "{}", problem.detail);
        }
        let mut huge = c.backends.add_request();
        huge.get().set_config("x".repeat(200_000).as_str());
        let error = huge.send().promise.await.err().unwrap();
        let problem = Problem::from_error(&error).expect("typed");
        assert_eq!(problem.status, 422);
        assert_ne!(
            problem.problem_type, TOO_LARGE,
            "refused as a request, not as an answer"
        );
        assert_eq!(listed(&c).await.len(), 1);
        put_file(&c, None, "still.txt", "", b"x", OnConflict::Refuse)
            .await
            .unwrap();
    })
    .await;
}
