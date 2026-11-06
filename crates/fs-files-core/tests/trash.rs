//! Trash, restore and purge.

mod common;

use common::{env, names, read_all, upload};
use fs_files_core::{Entry, Error, FileFilter, Files, Transfer};

#[tokio::test]
async fn trash_hides_restore_brings_back_purge_deletes() {
    let env = env().await;
    let e = &env.repo;
    let f = upload(e, None, "t.txt", "text/plain", b"bytes").await;
    e.trash(f.id).await.unwrap();
    assert!(names(e, None).await.is_empty(), "hidden from the folder");
    let found = e
        .find(
            &FileFilter {
                name: Some("t.t".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    assert!(found.items.is_empty(), "hidden from search");
    let trash = e
        .find(
            &FileFilter {
                trashed: true,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(trash.items.len(), 1);
    assert!(e.get(f.id).await.unwrap().unwrap().trashed);
    assert!(e.open(f.id).await.is_err(), "no reading from the trash");
    assert!(matches!(
        e.rename_file(f.id, "n").await,
        Err(Error::Conflict(_))
    ));
    assert!(matches!(e.trash(f.id).await, Err(Error::Conflict(_))));

    // restore needs the name to be free
    upload(e, None, "t.txt", "text/plain", b"other").await;
    assert!(matches!(e.restore(f.id).await, Err(Error::Conflict(_))));
    e.rename_file(
        e.find(
            &FileFilter {
                name: Some("t.txt".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap()
        .items
        .iter()
        .find_map(|i| match i {
            Entry::File(x) => Some(x.id),
            Entry::Folder(_) => None,
        })
        .unwrap(),
        "u.txt",
    )
    .await
    .unwrap();
    let back = e.restore(f.id).await.unwrap();
    assert!(!back.trashed);
    assert_eq!(read_all(e, f.id).await, b"bytes");
    assert!(matches!(e.restore(f.id).await, Err(Error::Conflict(_))));

    // purge only from the trash; then bytes and metadata are gone
    assert!(matches!(e.purge(f.id).await, Err(Error::Conflict(_))));
    e.trash(f.id).await.unwrap();
    e.purge(f.id).await.unwrap();
    assert!(e.get(f.id).await.unwrap().is_none());
    assert!(env.store.get(f.id).await.unwrap().is_none());
    let left: Vec<_> = walk(env.dir.path().join("blobs"));
    assert_eq!(
        left.len(),
        1,
        "only the other file's bytes remain: {left:?}"
    );
}

fn walk(dir: std::path::PathBuf) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(walk(p));
        } else {
            out.push(p);
        }
    }
    out
}
