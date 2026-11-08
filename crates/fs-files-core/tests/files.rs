//! Files: metadata, move, copy, rename, trash, search, paging.
#![allow(clippy::used_underscore_binding)] // tests: `_name` fixtures are kept alive on purpose and read by the harness

mod common;

use common::{env, names, read_all, upload};
use fs_files_core::{Error, Files};

#[tokio::test]
async fn a_file_is_an_as2_object_with_a_derived_name_and_iri() {
    let _env = env().await;
    let e = _env.repo.clone();
    let f = upload(&e, None, "a.png", "IMAGE/PNG", b"png?").await;
    assert_eq!(f.media_type, "image/png");
    assert_eq!(f.size, 4);
    assert_eq!(f.checksum, common::suite::checksum(b"png?"));
    assert!(f.iri.ends_with(&format!("/files/{}", f.id)));
    assert_eq!(e.get(f.id).await.unwrap().unwrap(), f);
}

#[tokio::test]
async fn the_envelope_carries_type_context_and_actor() {
    let env = env().await;
    let f = upload(&env.repo, None, "v.mp4", "video/mp4", b"x").await;
    let o = env.store.get(f.id).await.unwrap().unwrap();
    assert_eq!(o.envelope.object_type, "Video");
    assert_eq!(o.envelope.context.as_deref(), Some("files"));
    assert_eq!(o.envelope.name, "v.mp4");
    assert_eq!(o.envelope.attributed_to.as_deref(), Some(common::ME));
    let d = env.repo.create_folder(None, "d").await.unwrap();
    assert_eq!(
        env.store
            .get(d.id)
            .await
            .unwrap()
            .unwrap()
            .envelope
            .object_type,
        "Collection"
    );
    let doc = upload(&env.repo, None, "t.txt", "text/plain", b"x").await;
    assert_eq!(
        env.store
            .get(doc.id)
            .await
            .unwrap()
            .unwrap()
            .envelope
            .object_type,
        "Document"
    );
}

#[tokio::test]
async fn move_rename_and_copy() {
    let _env = env().await;
    let e = _env.repo.clone();
    let dir = e.create_folder(None, "dir").await.unwrap();
    let f = upload(&e, None, "a.txt", "text/plain", b"content").await;
    let moved = e.move_file(f.id, Some(dir.id)).await.unwrap();
    assert_eq!(moved.folder, Some(dir.id));
    assert_eq!(names(&e, None).await, ["dir"]);
    let renamed = e.rename_file(f.id, "b.txt").await.unwrap();
    assert_eq!(renamed.name, "b.txt");

    let same = e.copy_file(f.id, Some(dir.id)).await.unwrap();
    assert_eq!(same.name, "b (copy).txt");
    let again = e.copy_file(f.id, Some(dir.id)).await.unwrap();
    assert_eq!(again.name, "b (copy 2).txt");
    let elsewhere = e.copy_file(f.id, None).await.unwrap();
    assert_eq!(elsewhere.name, "b.txt");
    assert_ne!(elsewhere.id, f.id);
    assert_eq!(elsewhere.checksum, f.checksum);
    assert_eq!(read_all(&e, elsewhere.id).await, b"content");
    // the copy owns its bytes: purging the original leaves it readable
    e.trash(f.id).await.unwrap();
    e.purge(f.id).await.unwrap();
    assert_eq!(read_all(&e, elsewhere.id).await, b"content");

    let other = upload(&e, Some(dir.id), "c.txt", "text/plain", b"c").await;
    assert!(matches!(
        e.rename_file(other.id, "b (copy).txt").await,
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        e.rename_file(other.id, "x/y").await,
        Err(Error::InvalidName(_))
    ));
}
