//! Pages: the cursor of the wire is the cursor of the core, page after page, until none is left.

mod common;

use common::Served;
use fs_files_core::{Entry, FileFilter, Files, Sort};

fn names(items: &[Entry]) -> Vec<String> {
    items
        .iter()
        .map(|e| match e {
            Entry::File(f) => f.name.clone(),
            Entry::Folder(f) => f.name.clone(),
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_long_list_comes_in_pages_that_equal_the_providers() {
    let served = Served::start().await;
    for i in 0..130 {
        served.folder(None, &format!("f{i:03}")).await;
    }
    let files = served.client();
    let (mut seen, mut cursor, mut pages) = (Vec::new(), None, 0);
    loop {
        let wire = files.list(None, cursor).await.unwrap();
        let core = served.repo.list(None, cursor).await.unwrap();
        assert_eq!(names(&wire.items), names(&core.items));
        assert_eq!(wire.next, core.next, "the cursor is the provider's");
        seen.extend(names(&wire.items));
        pages += 1;
        match wire.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!((pages, seen.len()), (2, 130));
    assert_eq!(seen[0], "f000");
    assert_eq!(seen[129], "f129");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_pages_too_and_finds_the_trash_on_request() {
    let served = Served::start().await;
    for i in 0..105 {
        served.folder(None, &format!("hit{i:03}")).await;
    }
    let kept = served.file(None, "gone.txt", b"x").await;
    served.repo.trash(kept.id).await.unwrap();
    let files = served.client();
    let filter = FileFilter {
        name: Some("hit".into()),
        ..FileFilter::default()
    };
    let first = files
        .find_sorted(&filter, None, Sort::default())
        .await
        .unwrap();
    assert_eq!(first.items.len(), 100);
    let second = files
        .find_sorted(&filter, first.next, Sort::default())
        .await
        .unwrap();
    assert_eq!((second.items.len(), second.next), (5, None));
    let trash = FileFilter {
        trashed: true,
        ..FileFilter::default()
    };
    let page = files
        .find_sorted(&trash, None, Sort::default())
        .await
        .unwrap();
    assert_eq!(names(&page.items), ["gone.txt"]);
}
