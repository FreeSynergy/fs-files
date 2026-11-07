//! Unfinished uploads in pages cut in the store: every upload once, oldest first, the end said.

mod common;

use common::env;
use fs_files_core::{Cursor, Transfer};

#[tokio::test]
async fn pages_of_uploads_hold_each_upload_once_in_order() {
    let e = env().await;
    let mut ids = Vec::new();
    for i in 0..8 {
        let (_, id) = e
            .repo
            .start_upload(None, &format!("u{i}.bin"), "")
            .await
            .unwrap();
        ids.push(id);
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }
    let (mut seen, mut cursor, mut sizes) = (Vec::new(), None, Vec::new());
    loop {
        let page = e.repo.pending_uploads_page(cursor, 3).await.unwrap();
        sizes.push(page.items.len());
        seen.extend(page.items.into_iter().map(|u| u.id));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(sizes, [3, 3, 2]);
    assert_eq!(seen, ids);
    let past = e
        .repo
        .pending_uploads_page(Some(Cursor::at(50)), 3)
        .await
        .unwrap();
    assert!(past.items.is_empty() && past.next.is_none());
}
