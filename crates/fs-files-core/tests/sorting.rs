//! Sorting of `list` and `find`.

mod common;

use common::{env, upload};
use fs_files_core::{Entry, Error, FileFilter, Files, Sort, SortBy};

fn name_of(e: &Entry) -> &str {
    match e {
        Entry::File(f) => &f.name,
        Entry::Folder(f) => &f.name,
    }
}

async fn listed(r: &impl Files, sort: Sort) -> Vec<String> {
    r.list_sorted(None, None, sort)
        .await
        .unwrap()
        .items
        .iter()
        .map(|e| name_of(e).to_string())
        .collect()
}

fn by(by: SortBy, descending: bool) -> Sort {
    Sort::new(by, descending)
}

#[tokio::test]
async fn list_sorts_by_name_size_type_and_modified() {
    let e = env().await;
    let r = &e.repo;
    upload(r, None, "b.txt", "text/plain", b"12345").await;
    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    upload(r, None, "a.png", "image/png", b"1").await;
    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    upload(r, None, "C.pdf", "application/pdf", b"123").await;
    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    r.create_folder(None, "dir").await.unwrap();

    assert_eq!(
        listed(r, Sort::default()).await,
        ["a.png", "b.txt", "C.pdf", "dir"]
    );
    assert_eq!(
        listed(r, by(SortBy::Name, true)).await,
        ["dir", "C.pdf", "b.txt", "a.png"]
    );
    // size: a folder counts as 0, ties by name
    assert_eq!(
        listed(r, by(SortBy::Size, false)).await,
        ["dir", "a.png", "C.pdf", "b.txt"]
    );
    assert_eq!(
        listed(r, by(SortBy::Size, true)).await,
        ["b.txt", "C.pdf", "a.png", "dir"]
    );
    // type: application/pdf, image/png, inode/directory, text/plain
    assert_eq!(
        listed(r, by(SortBy::Type, false)).await,
        ["C.pdf", "a.png", "dir", "b.txt"]
    );
    assert!(matches!(
        r.list_sorted(None, None, by(SortBy::Type, true)).await,
        Err(Error::Unsupported(_))
    ));
    // modified: oldest first; a rename makes a file the newest
    assert_eq!(
        listed(r, by(SortBy::Modified, false)).await,
        ["b.txt", "a.png", "C.pdf", "dir"]
    );
    let b = r.list(None, None).await.unwrap().items;
    let id = match &b[1] {
        Entry::File(f) => f.id,
        Entry::Folder(f) => f.id,
    };
    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    r.rename_file(id, "b2.txt").await.unwrap();
    assert_eq!(
        listed(r, by(SortBy::Modified, true)).await,
        ["b2.txt", "dir", "C.pdf", "a.png"]
    );
}

#[tokio::test]
async fn sorted_pages_follow_the_order_and_search_sorts_too() {
    let e = env().await;
    let r = e.repo.clone().with_page_size(2);
    for (n, size) in [("e", 5), ("a", 1), ("d", 4), ("b", 2), ("c", 3)] {
        upload(&r, None, n, "text/plain", &vec![b'x'; size]).await;
    }
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = r
            .list_sorted(None, cursor, by(SortBy::Size, true))
            .await
            .unwrap();
        assert!(page.items.len() <= 2);
        seen.extend(page.items.iter().map(|e| name_of(e).to_string()));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, ["e", "d", "c", "b", "a"]);

    let filter = FileFilter {
        media_type: Some("text/".into()),
        ..Default::default()
    };
    let page = r
        .find_sorted(&filter, None, by(SortBy::Size, false))
        .await
        .unwrap();
    let names: Vec<_> = page.items.iter().map(name_of).collect();
    assert_eq!(names, ["a", "b"]);
    assert!(page.next.is_some());
}

#[tokio::test]
async fn folders_first_keeps_folders_before_files_over_page_borders_in_both_directions() {
    let e = env().await;
    let r = e.repo.clone().with_page_size(2);
    for n in ["b-file", "d-file", "a-file", "c-file"] {
        upload(&r, None, n, "text/plain", b"x").await;
    }
    for n in ["z-dir", "m-dir", "k-dir"] {
        r.create_folder(None, n).await.unwrap();
    }
    for descending in [false, true] {
        let sort = by(SortBy::Name, descending).with_folders_first();
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = r.list_sorted(None, cursor, sort).await.unwrap();
            assert!(page.items.len() <= 2);
            seen.extend(page.items.iter().map(|e| name_of(e).to_string()));
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let expected: Vec<&str> = if descending {
            vec![
                "z-dir", "m-dir", "k-dir", "d-file", "c-file", "b-file", "a-file",
            ]
        } else {
            vec![
                "k-dir", "m-dir", "z-dir", "a-file", "b-file", "c-file", "d-file",
            ]
        };
        assert_eq!(seen, expected, "descending: {descending}");
    }
}
