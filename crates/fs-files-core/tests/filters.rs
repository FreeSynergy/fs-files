//! Search filters by size and date.

mod common;

use chrono::{Duration, Utc};
use common::{env, upload};
use fs_files_core::{Entry, FileFilter, Files};

async fn found(r: &impl Files, f: FileFilter) -> Vec<String> {
    let mut names: Vec<_> = r
        .find(&f, None)
        .await
        .unwrap()
        .items
        .iter()
        .map(|e| match e {
            Entry::File(f) => f.name.clone(),
            Entry::Folder(f) => f.name.clone(),
        })
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn size_filters_are_inclusive_and_a_folder_counts_as_zero() {
    let e = env().await;
    let r = &e.repo;
    upload(r, None, "empty", "text/plain", b"").await;
    upload(r, None, "small", "text/plain", &[b'x'; 10]).await;
    upload(r, None, "big", "text/plain", &[b'x'; 1000]).await;
    r.create_folder(None, "dir").await.unwrap();
    let f = |min, max| FileFilter {
        min_size: min,
        max_size: max,
        ..Default::default()
    };
    assert_eq!(found(r, f(Some(10), None)).await, ["big", "small"]);
    assert_eq!(found(r, f(None, Some(10))).await, ["dir", "empty", "small"]);
    assert_eq!(found(r, f(Some(10), Some(999))).await, ["small"]);
    assert_eq!(found(r, f(Some(1), Some(0))).await, Vec::<String>::new());
    assert_eq!(found(r, f(Some(1001), None)).await, Vec::<String>::new());
    // combined with the other filters
    let both = FileFilter {
        name: Some("BI".into()),
        min_size: Some(500),
        ..Default::default()
    };
    assert_eq!(found(r, both).await, ["big"]);
}

#[tokio::test]
async fn date_filters_split_old_from_new() {
    let e = env().await;
    let r = &e.repo;
    upload(r, None, "old", "text/plain", b"1").await;
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let cut = Utc::now();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    upload(r, None, "new", "text/plain", b"2").await;

    let f = |after, before| FileFilter {
        created_after: after,
        created_before: before,
        ..Default::default()
    };
    assert_eq!(found(r, f(Some(cut), None)).await, ["new"]);
    assert_eq!(found(r, f(None, Some(cut))).await, ["old"]);
    assert_eq!(
        found(
            r,
            f(
                Some(cut - Duration::hours(1)),
                Some(cut + Duration::hours(1))
            )
        )
        .await,
        ["new", "old"]
    );
    assert_eq!(
        found(r, f(Some(cut + Duration::hours(1)), None)).await,
        Vec::<String>::new()
    );

    // modified: a rename of the old file makes it recent
    let old = found(
        r,
        FileFilter {
            name: Some("old".into()),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(old, ["old"]);
    let page = r.list(None, None).await.unwrap();
    let old_id = page
        .items
        .iter()
        .find_map(|e| match e {
            Entry::File(f) if f.name == "old" => Some(f.id),
            _ => None,
        })
        .unwrap();
    r.rename_file(old_id, "older").await.unwrap();
    let m = FileFilter {
        modified_after: Some(cut),
        ..Default::default()
    };
    assert_eq!(found(r, m).await, ["new", "older"]);
    let created = FileFilter {
        created_after: Some(cut),
        ..Default::default()
    };
    assert_eq!(
        found(r, created).await,
        ["new"],
        "a rename does not change the creation time"
    );
}
