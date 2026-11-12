//! Searching over the wire: filters, sort orders, ties.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::search::{fill, find, find_with};
use common::{caps, is, set_id, with_server};
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
use fs_interfaces::files_capnp::{SortBy, file_filter};

#[tokio::test(flavor = "current_thread")]
async fn a_filter_is_the_and_of_its_members() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let dir = fill(&c).await;
        let all = |_: file_filter::Builder<'_>| {};
        assert_eq!(
            find(&c, all, None).await.unwrap(),
            ["Photos", "cat.png", "notes.txt", "trip.txt"],
            "folders first, each group by name"
        );
        let by_name = find(&c, |mut f| f.set_name("TXT"), None).await.unwrap();
        assert_eq!(
            by_name,
            ["notes.txt", "trip.txt"],
            "a substring of the name"
        );
        let images = find(&c, |mut f| f.set_media_type("image/"), None)
            .await
            .unwrap();
        assert_eq!(images, ["cat.png"]);
        let inside = find(
            &c,
            move |mut f| set_id(f.reborrow().init_folder(), dir),
            None,
        )
        .await;
        assert_eq!(inside.unwrap(), ["cat.png", "trip.txt"]);
        let sized = find(
            &c,
            |mut f| {
                f.set_min_size(2);
                f.set_max_size(299);
            },
            None,
        )
        .await;
        assert_eq!(sized.unwrap(), ["notes.txt"]);
        let both = find(
            &c,
            |mut f| {
                f.set_name("t");
                f.set_media_type("text/");
                f.set_max_size(1);
            },
            None,
        );
        assert_eq!(both.await.unwrap(), ["trip.txt"]);
        let later = find(
            &c,
            |f| f.init_created_after().set_unix_millis(4_000_000_000_000),
            None,
        );
        assert!(later.await.unwrap().is_empty());
        let early = find(
            &c,
            |f| f.init_created_before().set_unix_millis(4_000_000_000_000),
            None,
        );
        assert_eq!(early.await.unwrap().len(), 4);
        let changed = |f: file_filter::Builder<'_>| f.init_modified_after().set_unix_millis(0);
        assert_eq!(find(&c, changed, None).await.unwrap().len(), 4);
        let unchanged = |f: file_filter::Builder<'_>| f.init_modified_before().set_unix_millis(0);
        assert!(find(&c, unchanged, None).await.unwrap().is_empty());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn sort_orders_ties_and_the_one_that_is_ascending_only() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        fill(&c).await;
        let sorted = |by, desc| find(&c, |_| {}, Some((by, desc)));
        assert_eq!(
            sorted(SortBy::Size, false).await.unwrap(),
            ["Photos", "trip.txt", "notes.txt", "cat.png"]
        );
        assert_eq!(
            sorted(SortBy::Size, true).await.unwrap(),
            ["Photos", "cat.png", "notes.txt", "trip.txt"]
        );
        assert_eq!(
            sorted(SortBy::Name, true).await.unwrap(),
            ["Photos", "trip.txt", "notes.txt", "cat.png"]
        );
        assert_eq!(
            sorted(SortBy::MediaType, false).await.unwrap(),
            ["Photos", "cat.png", "notes.txt", "trip.txt"],
            "folders first; then image/png, text/plain twice (by name)"
        );
        let modified = sorted(SortBy::Modified, true).await.unwrap();
        assert_eq!(modified[0], "Photos", "folders first");
        assert_eq!(modified[1], "cat.png", "then the last file made, first");
        let error = sorted(SortBy::MediaType, true).await.err().unwrap();
        is(&error, UNSUPPORTED_PROBLEM_TYPE, 501);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn folders_first_is_the_wish_of_the_client_and_on_when_it_says_nothing() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        fill(&c).await;
        let all = |_: file_filter::Builder<'_>| {};
        // a client that never writes the field (old client): folders first
        assert_eq!(
            find(&c, all, None).await.unwrap(),
            ["Photos", "cat.png", "notes.txt", "trip.txt"]
        );
        // asked for in so many words: folders first
        assert_eq!(
            find_with(&c, all, Some((SortBy::Name, false)), true)
                .await
                .unwrap(),
            ["Photos", "cat.png", "notes.txt", "trip.txt"]
        );
        // counter test: `false` is kept on the wire and gives the mixed order by name
        assert_eq!(
            find_with(&c, all, Some((SortBy::Name, false)), false)
                .await
                .unwrap(),
            ["cat.png", "notes.txt", "Photos", "trip.txt"]
        );
    })
    .await;
}
