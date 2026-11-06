//! Search and paging.

mod common;

use common::{env, upload};
use fs_files_core::{Entry, FileFilter, Files};

#[tokio::test]
async fn search_combines_name_media_type_and_folder() {
    let _env = env().await;
    let e = _env.repo.clone();
    let d = e.create_folder(None, "Holiday").await.unwrap();
    upload(&e, Some(d.id), "Beach.PNG", "image/png", b"1").await;
    upload(&e, Some(d.id), "beach.txt", "text/plain", b"2").await;
    upload(&e, None, "Sea.jpg", "image/jpeg", b"3").await;
    let find = |f: FileFilter| {
        let e = e.clone();
        async move { e.find(&f, None).await.unwrap().items.len() }
    };
    assert_eq!(
        find(FileFilter {
            name: Some("BEACH".into()),
            ..Default::default()
        })
        .await,
        2
    );
    assert_eq!(
        find(FileFilter {
            media_type: Some("image/".into()),
            ..Default::default()
        })
        .await,
        2
    );
    assert_eq!(
        find(FileFilter {
            name: Some("beach".into()),
            media_type: Some("image/".into()),
            ..Default::default()
        })
        .await,
        1
    );
    assert_eq!(
        find(FileFilter {
            folder: Some(d.id),
            ..Default::default()
        })
        .await,
        2
    );
    assert_eq!(
        find(FileFilter {
            name: Some("holi".into()),
            ..Default::default()
        })
        .await,
        1,
        "folders are found by name"
    );
    assert_eq!(find(FileFilter::default()).await, 4);
}

#[tokio::test]
async fn pages_follow_the_cursor() {
    let _env = env().await;
    let e = _env.repo.clone().with_page_size(2);
    for n in ["a", "b", "c", "d", "e"] {
        upload(&e, None, n, "text/plain", n.as_bytes()).await;
    }
    let mut seen = vec![];
    let mut cursor = None;
    loop {
        let page = e.list(None, cursor).await.unwrap();
        assert!(page.items.len() <= 2);
        seen.extend(page.items.iter().map(|i| match i {
            Entry::File(f) => f.name.clone(),
            Entry::Folder(f) => f.name.clone(),
        }));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, ["a", "b", "c", "d", "e"]);
}
