//! Pages over the wire: a long folder in whole pages, a bad cursor refused, a search text refused at the door.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::search::{fill, find};
use common::{caps, is, names, text_file, with_server};
use fs_interfaces::rpc::{INVALID_REQUEST, write_uuid};

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

#[tokio::test(flavor = "current_thread")]
async fn a_cursor_beyond_the_end_gives_an_empty_page_and_one_beyond_any_list_is_refused() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        text_file(&c, None, "only.txt", "x").await;
        let list = |place: u64| {
            let mut cursor = vec![1u8];
            cursor.extend(place.to_le_bytes());
            let mut call = c.folders.list_request();
            call.get().init_cursor().set_opaque(&cursor);
            call.send().promise
        };
        for place in [1u64, 2, 1 << 20, u64::from(u32::MAX)] {
            let answer = list(place).await.unwrap_or_else(|e| panic!("{place}: {e}"));
            let page = answer.get().unwrap().get_page().unwrap();
            assert_eq!(page.get_items().unwrap().len(), 0, "{place}");
            assert!(page.get_next().unwrap().get_opaque().unwrap().is_empty());
        }
        for place in [1u64 << 32, 1 << 62, u64::MAX - 1, u64::MAX] {
            is(&list(place).await.err().unwrap(), INVALID_REQUEST, 422);
        }
    })
    .await;
}
