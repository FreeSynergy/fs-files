//! Listing and searching over the wire: sort orders, filters, paging; bad input is refused typed.

mod common;

use common::{Caps, caps, is, mkdir, names, read_entry, set_id, text_file, with_server};
use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
use fs_interfaces::files_capnp::{OnConflict, SortBy, file_filter};
use fs_interfaces::rpc::{INVALID_REQUEST, write_uuid};

async fn find(
    c: &Caps,
    filter: impl Fn(file_filter::Builder<'_>),
    sort: Option<(SortBy, bool)>,
) -> capnp::Result<Vec<String>> {
    let (mut out, mut cursor) = (vec![], vec![]);
    loop {
        let mut call = c.search.find_request();
        filter(call.get().init_filter());
        call.get().init_cursor().set_opaque(&cursor);
        if let Some((by, descending)) = sort {
            let mut order = call.get().init_sort();
            order.set_by(by);
            order.set_descending(descending);
        }
        let answer = call.send().promise.await?;
        let page = answer.get()?.get_page()?;
        out.extend(page.get_items()?.iter().map(|e| read_entry(e).0));
        cursor = page.get_next()?.get_opaque()?.to_vec();
        if cursor.is_empty() {
            return Ok(out);
        }
    }
}

/// A folder `Photos` with two files, and a file of 5 bytes beside it.
async fn fill(c: &Caps) -> uuid::Uuid {
    let dir = mkdir(c, None, "Photos").await.unwrap();
    text_file(c, None, "notes.txt", "12345").await;
    text_file(c, Some(dir), "trip.txt", "1").await;
    let png = vec![7; 300];
    let put = common::put_file(
        c,
        Some(dir),
        "cat.png",
        "image/png",
        &png,
        OnConflict::Refuse,
    );
    put.await.unwrap();
    dir
}

#[tokio::test(flavor = "current_thread")]
async fn a_filter_is_the_and_of_its_members() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let dir = fill(&c).await;
        let all = |_: file_filter::Builder<'_>| {};
        assert_eq!(
            find(&c, all, None).await.unwrap(),
            ["cat.png", "notes.txt", "Photos", "trip.txt"]
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
            ["cat.png", "notes.txt", "trip.txt", "Photos"]
        );
        assert_eq!(
            sorted(SortBy::Name, true).await.unwrap(),
            ["trip.txt", "Photos", "notes.txt", "cat.png"]
        );
        assert_eq!(
            sorted(SortBy::MediaType, false).await.unwrap(),
            ["cat.png", "Photos", "notes.txt", "trip.txt"],
            "image/png, inode/directory, text/plain twice (by name)"
        );
        let modified = sorted(SortBy::Modified, true).await.unwrap();
        assert_eq!(modified[0], "cat.png", "the last one made, first");
        let error = sorted(SortBy::MediaType, true).await.err().unwrap();
        is(&error, UNSUPPORTED_PROBLEM_TYPE, 501);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pages_of_a_long_folder_are_whole_and_a_bad_cursor_is_refused() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        for n in 0..230 {
            text_file(&c, None, &format!("f-{n:03}.txt"), "x").await;
        }
        let listed = common::list_all(&c, None).await;
        let expected: Vec<String> = (0..230).map(|n| format!("f-{n:03}.txt")).collect();
        assert_eq!(
            names(&listed),
            expected.iter().map(String::as_str).collect::<Vec<_>>()
        );

        let mut first = c.folders.list_request();
        write_uuid(first.get().init_folder(), None);
        let answer = first.send().promise.await.unwrap();
        let page = answer.get().unwrap().get_page().unwrap();
        assert_eq!(
            page.get_items().unwrap().len(),
            100,
            "a page is 100 entries"
        );

        for bad in [&b"bad"[..], &[2, 0, 0, 0, 0, 0, 0, 0, 0][..], &[1, 0][..]] {
            let mut call = c.folders.list_request();
            call.get().init_cursor().set_opaque(bad);
            is(
                &call.send().promise.await.err().unwrap(),
                INVALID_REQUEST,
                422,
            );
        }
        let past = find(&c, |_| {}, None).await.unwrap();
        assert_eq!(past.len(), 230);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_max_size_below_minus_one_and_a_huge_search_text_are_refused_at_the_door() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        fill(&c).await;
        let error = find(&c, |mut f| f.set_max_size(-2), None)
            .await
            .err()
            .unwrap();
        is(&error, INVALID_REQUEST, 422);
        let huge = "x".repeat(10 * 1024 * 1024);
        let error = find(&c, |mut f| f.set_name(huge.as_str()), None)
            .await
            .err()
            .unwrap();
        is(&error, INVALID_REQUEST, 422);
        assert_eq!(
            find(&c, |_| {}, None).await.unwrap().len(),
            4,
            "still connected"
        );
    })
    .await;
}
