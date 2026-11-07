//! `subscribe` over a real socket: every change of a file or folder is told with its envelope, until
//! the consumer drops the subscription; a provider without a feed says so; a feed that cannot go on says so.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use capnp_rpc::new_client;
use common::stub::Stub;
use common::{caps, is, mkdir, set_id, text_file, with_server, with_stub};
use fs_files_core::FileChange;
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
use fs_interfaces::common_capnp::{change, change_listener};
use fs_interfaces::rpc::{FEED_ENDED_PROBLEM_TYPE, Problem, read_envelope};
use fs_store::ObjectId;

#[derive(Clone, Default)]
struct Seen {
    told: Rc<RefCell<Vec<String>>>,
    ended: Rc<RefCell<Vec<Problem>>>,
}

impl change_listener::Server for Seen {
    async fn change(self: Rc<Self>, params: change_listener::ChangeParams) -> capnp::Result<()> {
        let told = match params.get()?.get_change()?.which()? {
            change::Which::Created(e) => {
                let e = read_envelope(e?)?;
                format!("created {} {}", e.object_type, e.name)
            }
            change::Which::Updated(e) => {
                let e = read_envelope(e?)?;
                format!("updated {} {}", e.object_type, e.name)
            }
            change::Which::Deleted(_) => "deleted".to_string(),
        };
        self.told.borrow_mut().push(told);
        Ok(())
    }

    async fn ended(
        self: Rc<Self>,
        params: change_listener::EndedParams,
        _: change_listener::EndedResults,
    ) -> capnp::Result<()> {
        let problem = Problem::read(params.get()?.get_problem()?)?;
        self.ended.borrow_mut().push(problem);
        Ok(())
    }
}

async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
}

#[tokio::test(flavor = "current_thread")]
async fn subscribers_see_changes_with_their_envelope_until_they_drop_the_capability() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let seen = Seen::default();
        let listener: change_listener::Client = new_client(seen.clone());
        let mut subscribe = root.subscribe_request();
        subscribe.get().set_listener(listener);
        let answer = subscribe.send().promise.await.unwrap();
        let subscription = answer.get().unwrap().get_subscription().unwrap();

        let dir = mkdir(&c, None, "Docs").await.unwrap();
        let file = text_file(&c, Some(dir), "a.txt", "x").await;
        let mut rename = c.folders.rename_file_request();
        set_id(rename.get().init_id(), file.id);
        rename.get().set_name("b.txt");
        rename.send().promise.await.unwrap();
        let mut trash = c.folders.trash_request();
        set_id(trash.get().init_id(), file.id);
        trash.send().promise.await.unwrap();
        let mut purge = c.folders.purge_request();
        set_id(purge.get().init_id(), file.id);
        purge.send().promise.await.unwrap();
        std::fs::write(fx.root.join("Docs/found.txt"), "outside").unwrap();
        c.folders.rescan_request().send().promise.await.unwrap();
        settle().await;
        assert_eq!(
            *seen.told.borrow(),
            [
                "created Collection Docs",
                "created Document a.txt",
                "updated Document b.txt",
                "updated Document b.txt",
                "deleted",
                "created Document found.txt",
            ]
        );

        drop(subscription);
        drop(answer);
        settle().await;
        mkdir(&c, None, "Unwatched").await.unwrap();
        settle().await;
        assert_eq!(seen.told.borrow().len(), 6, "still told after the drop");
        assert!(seen.ended.borrow().is_empty());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_provider_without_a_feed_says_so_and_a_feed_that_cannot_go_on_ends_with_a_reason() {
    with_stub(Stub::default(), |root| async move {
        let listener: change_listener::Client = new_client(Seen::default());
        let mut subscribe = root.subscribe_request();
        subscribe.get().set_listener(listener);
        let error = subscribe.send().promise.await.err().unwrap();
        is(&error, UNSUPPORTED_PROBLEM_TYPE, 501);
        assert!(
            Problem::from_error(&error)
                .unwrap()
                .detail
                .contains("file.subscribe")
        );
    })
    .await;

    let stub = Stub {
        with_feed: true,
        fail_reads: true,
        ..Stub::default()
    };
    let feed = stub.feed.clone();
    with_stub(stub, |root| async move {
        let seen = Seen::default();
        let mut subscribe = root.subscribe_request();
        subscribe
            .get()
            .set_listener(new_client::<change_listener::Client, _>(seen.clone()));
        let _answer = subscribe.send().promise.await.unwrap();
        feed.notify(&FileChange::Created(ObjectId::new()));
        settle().await;
        let ended = seen.ended.borrow();
        assert_eq!(ended.len(), 1, "told that it ended, not left waiting");
        assert_eq!(
            (ended[0].problem_type.as_str(), ended[0].status),
            (FEED_ENDED_PROBLEM_TYPE, 410)
        );
        assert!(seen.told.borrow().is_empty());
    })
    .await;
}
