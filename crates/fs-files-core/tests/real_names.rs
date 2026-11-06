//! Files under their real names on a real temporary directory: disk and database stay in step.

mod common;

use common::disk::{exists, is_conflict, on_disk, root, tree};
use common::{env, names, read_all, upload};
use fs_files_core::{BlobBackend, Error, Files, LocalFolder, Spot, Transfer};

#[tokio::test]
async fn files_and_folders_follow_every_operation_on_the_disk() {
    let e = env().await;
    let r = &e.repo;
    let urlaub = r.create_folder(None, "Urlaub").await.unwrap();
    assert!(root(&e).join("Urlaub").is_dir());
    let sub = r.create_folder(Some(urlaub.id), "Strand").await.unwrap();
    let foto = upload(r, Some(urlaub.id), "Foto.jpg", "image/jpeg", b"jpeg bytes").await;
    assert_eq!(on_disk(&e, "Urlaub/Foto.jpg"), b"jpeg bytes");
    assert_eq!(read_all(r, foto.id).await, b"jpeg bytes");

    // rename
    let foto = r.rename_file(foto.id, "Bild.jpg").await.unwrap();
    assert!(!exists(&e, "Urlaub/Foto.jpg"));
    assert_eq!(on_disk(&e, "Urlaub/Bild.jpg"), b"jpeg bytes");
    // move
    let foto = r.move_file(foto.id, Some(sub.id)).await.unwrap();
    assert_eq!(on_disk(&e, "Urlaub/Strand/Bild.jpg"), b"jpeg bytes");
    assert!(!exists(&e, "Urlaub/Bild.jpg"));
    // copy gets a free name, both stay readable
    let copy = r.copy_file(foto.id, Some(sub.id)).await.unwrap();
    assert_eq!(copy.name, "Bild (copy).jpg");
    assert_eq!(on_disk(&e, "Urlaub/Strand/Bild (copy).jpg"), b"jpeg bytes");
    // folder rename moves the directory; everything below stays readable
    r.rename_folder(urlaub.id, "Ferien").await.unwrap();
    assert!(!exists(&e, "Urlaub"));
    assert_eq!(on_disk(&e, "Ferien/Strand/Bild.jpg"), b"jpeg bytes");
    assert_eq!(read_all(r, foto.id).await, b"jpeg bytes");
    assert_eq!(read_all(r, copy.id).await, b"jpeg bytes");
    // folder move
    let archiv = r.create_folder(None, "Archiv").await.unwrap();
    r.move_folder(sub.id, Some(archiv.id)).await.unwrap();
    assert_eq!(on_disk(&e, "Archiv/Strand/Bild.jpg"), b"jpeg bytes");
    assert!(!exists(&e, "Ferien/Strand"));
    assert_eq!(read_all(r, foto.id).await, b"jpeg bytes");
    // trash: gone from the folder, kept in the program's own folder, not as a user file
    r.trash(foto.id).await.unwrap();
    assert!(!exists(&e, "Archiv/Strand/Bild.jpg"));
    assert!(
        root(&e)
            .join(format!(".fs-files/trash/{}", foto.id))
            .is_file()
    );
    assert_eq!(tree(&root(&e)).len(), 4); // Ferien, Archiv, Archiv/Strand, Bild (copy).jpg
    // restore
    let back = r.restore(foto.id).await.unwrap();
    assert_eq!(back.name, "Bild.jpg");
    assert_eq!(on_disk(&e, "Archiv/Strand/Bild.jpg"), b"jpeg bytes");
    assert!(
        !root(&e)
            .join(format!(".fs-files/trash/{}", foto.id))
            .exists()
    );
    // purge removes the bytes
    r.trash(copy.id).await.unwrap();
    r.purge(copy.id).await.unwrap();
    assert!(!exists(&e, "Archiv/Strand/Bild (copy).jpg"));
    assert!(
        std::fs::read_dir(root(&e).join(".fs-files/trash"))
            .unwrap()
            .next()
            .is_none()
    );
    // folder delete removes the empty directory
    r.delete_folder(archiv.id).await.unwrap_err(); // not empty
    r.trash(foto.id).await.unwrap();
    r.purge(foto.id).await.unwrap();
    r.delete_folder(sub.id).await.unwrap();
    r.delete_folder(archiv.id).await.unwrap();
    assert!(!exists(&e, "Archiv"));
}

#[tokio::test]
async fn unicode_long_spaces_and_case_differences_work() {
    let e = env().await;
    let r = &e.repo;
    let long = format!("{}.txt", "l".repeat(251));
    for n in [
        "Größe & Maße.txt",
        "日本語 の ファイル.txt",
        "with  two  spaces",
        ".dotfile",
        &long,
    ] {
        let f = upload(r, None, n, "text/plain", n.as_bytes()).await;
        assert_eq!(on_disk(&e, n), n.as_bytes());
        assert_eq!(read_all(r, f.id).await, n.as_bytes());
    }
    // Linux: different case is a different file; an exact duplicate is refused
    let a = upload(r, None, "Foto.jpg", "image/jpeg", b"upper").await;
    let b = upload(r, None, "foto.jpg", "image/jpeg", b"lower").await;
    assert_eq!(read_all(r, a.id).await, b"upper");
    assert_eq!(read_all(r, b.id).await, b"lower");
    assert_eq!(on_disk(&e, "Foto.jpg"), b"upper");
    assert_eq!(on_disk(&e, "foto.jpg"), b"lower");
    assert!(is_conflict(
        r.start_upload(None, "Foto.jpg", "image/jpeg")
            .await
            .map(|_| ())
    ));
    // a name over 255 bytes is refused before anything exists
    assert!(matches!(
        r.start_upload(None, &"x".repeat(256), "text/plain").await,
        Err(Error::InvalidName(_))
    ));
}

#[tokio::test]
async fn hostile_names_never_escape_or_create_anything() {
    let e = env().await;
    let r = &e.repo;
    let outside_marker = e.dir.path().join("escaped");
    let bad = [
        "../escaped",
        "a/b",
        "/abs",
        ".",
        "..",
        "a\0b",
        "",
        " ",
        "x\\y",
        "..\\escaped",
        "tab\there",
        ".fs-files",
    ];
    let before = tree(&root(&e));
    for n in bad {
        assert!(r.create_folder(None, n).await.is_err(), "folder {n:?}");
        assert!(
            r.start_upload(None, n, "text/plain").await.is_err(),
            "upload {n:?}"
        );
    }
    let f = upload(r, None, "ok.txt", "text/plain", b"x").await;
    let d = r.create_folder(None, "dir").await.unwrap();
    let before = {
        let mut b = before;
        b.push("dir".into());
        b.push("ok.txt".into());
        b.sort();
        b
    };
    for n in bad {
        assert!(r.rename_file(f.id, n).await.is_err(), "rename {n:?}");
        assert!(
            r.rename_folder(d.id, n).await.is_err(),
            "rename folder {n:?}"
        );
    }
    assert_eq!(tree(&root(&e)), before);
    assert!(!outside_marker.exists());
    let all: Vec<_> = tree(e.dir.path())
        .into_iter()
        .filter(|p| !p.starts_with("blobs/.fs-files"))
        .collect();
    assert_eq!(
        all,
        ["blobs", "blobs/dir", "blobs/ok.txt"].map(String::from)
    );
    // and the backend itself refuses hostile keys as names
    let b = LocalFolder::open(root(&e)).await.unwrap();
    for k in [
        "../escaped",
        "a/../../escaped",
        "/etc/x",
        ".fs-files/trash/x",
        "a//b",
    ] {
        assert!(b.make_dir(k, true).await.is_err(), "{k}");
        assert!(
            b.relocate(&Spot::Path("ok.txt".into()), &Spot::Path(k.into()))
                .await
                .is_err(),
            "{k}"
        );
    }
    assert!(
        b.relocate(&Spot::Path("ok.txt".into()), &Spot::Trash("../x".into()))
            .await
            .is_err()
    );
    assert!(!outside_marker.exists());
    assert_eq!(on_disk(&e, "ok.txt"), b"x");
}

#[tokio::test]
async fn a_path_the_disk_cannot_hold_fails_cleanly() {
    let e = env().await;
    let r = &e.repo;
    let mut parent = None;
    let mut failed = false;
    for i in 0..30 {
        match r
            .create_folder(parent, &format!("{i:02}{}", "d".repeat(200)))
            .await
        {
            Ok(f) => parent = Some(f.id),
            Err(err) => {
                assert!(
                    matches!(err, Error::InvalidKey(_) | Error::InvalidName(_)),
                    "{err}"
                );
                failed = true;
                break;
            }
        }
    }
    assert!(failed, "the path limit must be reached");
    // nothing half done: the failed folder is in neither place
    let listed = names(r, parent).await;
    assert!(listed.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_planted_in_the_root_leads_nowhere() {
    let e = env().await;
    let r = &e.repo;
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"secret").unwrap();
    let d = r.create_folder(None, "dir").await.unwrap();
    let f = upload(r, Some(d.id), "a.txt", "text/plain", b"a").await;
    // somebody replaces the folder by a link to the outside
    std::fs::rename(root(&e).join("dir"), e.dir.path().join("moved-away")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root(&e).join("dir")).unwrap();
    assert!(r.open(f.id).await.is_err());
    assert!(
        r.start_upload(Some(d.id), "b.txt", "text/plain")
            .await
            .is_err()
    );
    assert!(r.rename_file(f.id, "c.txt").await.is_err());
    assert!(r.trash(f.id).await.is_err());
    assert!(r.create_folder(Some(d.id), "sub").await.is_err());
    assert!(r.rescan(Some(d.id)).await.is_err());
    let mut seen: Vec<_> = std::fs::read_dir(outside.path())
        .unwrap()
        .map(|x| x.unwrap().file_name())
        .collect();
    seen.sort();
    assert_eq!(seen, ["secret"]);
    assert_eq!(
        std::fs::read(outside.path().join("secret")).unwrap(),
        b"secret"
    );
}
