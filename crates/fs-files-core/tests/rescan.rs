//! Changes made from outside the program: `rescan` and what `open` says.

mod common;

use common::disk::root;
use common::{env, read_all, upload};
use fs_files_core::{BlobBackend, Entry, Files, LocalFolder, Transfer};
use fs_objects::ObjectId;

#[tokio::test]
async fn rescan_takes_over_new_files_and_folders_and_updates_changed_ones() {
    let e = env().await;
    let r = &e.repo;
    let d = r.create_folder(None, "d").await.unwrap();
    let f = upload(r, Some(d.id), "a.txt", "text/plain", b"one").await;
    // first rescan records the modification time and finds nothing to change
    let first = r.rescan(None).await.unwrap();
    assert!(
        first.updated.len() <= 1 && first.added.is_empty() && first.missing.is_empty(),
        "{first:?}"
    );
    let quiet = r.rescan(None).await.unwrap();
    assert!(
        quiet.updated.is_empty() && quiet.added.is_empty() && quiet.missing.is_empty(),
        "{quiet:?}"
    );

    std::fs::write(root(&e).join("d/a.txt"), b"changed!").unwrap();
    std::fs::write(root(&e).join("d/new.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    std::fs::create_dir_all(root(&e).join("fresh/inner")).unwrap();
    std::fs::write(root(&e).join("fresh/inner/deep.txt"), b"deep").unwrap();
    // open refuses a file whose size no longer matches
    let err = r.open(f.id).await.err().expect("size mismatch is an error");
    assert!(err.to_string().contains("rescan"), "{err}");

    let report = r.rescan(None).await.unwrap();
    assert_eq!(report.updated.len(), 1);
    assert_eq!(report.updated[0].size, 8);
    assert_eq!(
        report.updated[0].checksum,
        common::suite::checksum(b"changed!")
    );
    assert_eq!(report.added.len(), 4, "{report:?}"); // new.png, fresh, inner, deep.txt
    assert_eq!(read_all(r, f.id).await, b"changed!");
    let png = report
        .added
        .iter()
        .find_map(|x| match x {
            Entry::File(f) if f.name == "new.png" => Some(f),
            _ => None,
        })
        .unwrap();
    assert_eq!(png.media_type, "image/png");
    assert_eq!(png.folder, Some(d.id));
    let fresh = r.list(None, None).await.unwrap();
    assert!(
        fresh
            .items
            .iter()
            .any(|x| matches!(x, Entry::Folder(f) if f.name == "fresh"))
    );

    // a file that vanished is reported, its record is kept, nothing is deleted
    std::fs::remove_file(root(&e).join("d/new.png")).unwrap();
    let gone = r.rescan(Some(d.id)).await.unwrap();
    assert_eq!(gone.missing.len(), 1);
    assert_eq!(gone.missing[0].name, "new.png");
    assert!(r.get(gone.missing[0].id).await.unwrap().is_some());
    let err = r.open(gone.missing[0].id).await.err().unwrap();
    assert!(
        err.problem().unwrap().kind.ends_with("changed-outside"),
        "{err}"
    );

    // odd entries are listed, not taken over
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc", root(&e).join("d/link")).unwrap();
    std::fs::write(root(&e).join("d/ok.txt"), b"ok").unwrap();
    let odd = r.rescan(Some(d.id)).await.unwrap();
    assert!(
        odd.skipped
            .iter()
            .any(|s| s.contains("d/link") && s.contains("symlink")),
        "{odd:?}"
    );
    assert_eq!(odd.added.len(), 1);
}

#[tokio::test]
async fn rescan_ignores_the_id_keyed_files_of_the_first_version() {
    let e = env().await;
    let b = LocalFolder::open(root(&e)).await.unwrap();
    let id = ObjectId::new().to_string();
    let key = format!("{}/{id}", &id[id.len() - 2..]);
    b.put(&key, 0, b"old").await.unwrap();
    b.commit(&key).await.unwrap();
    let report = e.repo.rescan(None).await.unwrap();
    assert!(report.added.is_empty(), "{report:?}");
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(b.get(&key, 0, 9).await.unwrap(), b"old");
}
