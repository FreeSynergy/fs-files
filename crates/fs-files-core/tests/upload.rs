//! Resumable upload: offsets, restart, finish, abort, large files.

mod common;

use common::suite::{bytes, checksum};
use common::{env, names, read_all, reopen, upload};
use fs_files_core::{Error, Transfer};

#[tokio::test]
async fn a_chunk_at_the_wrong_offset_is_an_error() {
    let _env = env().await;
    let e = _env.repo.clone();
    let (up, _) = e
        .start_upload(None, "a.bin", "application/octet-stream")
        .await
        .unwrap();
    up.write(0, b"abc").await.unwrap();
    for bad in [0, 1, 2, 4] {
        assert!(
            matches!(
                up.write(bad, b"x").await,
                Err(Error::Offset { expected: 3, .. })
            ),
            "{bad}"
        );
    }
    assert_eq!(up.received().await.unwrap(), 3);
    up.write(3, b"def").await.unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(
        (f.size, f.checksum.as_str()),
        (6, checksum(b"abcdef").as_str())
    );
    assert_eq!(read_all(&e, f.id).await, b"abcdef");
}

#[tokio::test]
async fn nothing_exists_until_finish() {
    let _env = env().await;
    let e = _env.repo.clone();
    let (up, _) = e
        .start_upload(None, "later.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"half").await.unwrap();
    assert!(
        names(&e, None).await.is_empty(),
        "not listed while uploading"
    );
    up.finish().await.unwrap();
    assert_eq!(names(&e, None).await, ["later.txt"]);
}

#[tokio::test]
async fn an_upload_survives_a_restart_of_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let data = bytes(2_500_000);
    let half = 1_200_000;
    let id = {
        let (repo, _) = reopen(dir.path(), false).await;
        let (up, id) = repo
            .start_upload(None, "big.bin", "application/x-big")
            .await
            .unwrap();
        for (i, c) in data[..half].chunks(400_000).enumerate() {
            up.write((i * 400_000) as u64, c).await.unwrap();
        }
        id
    }; // store, repository and backend dropped: "the program ended"
    let (repo, _) = reopen(dir.path(), false).await;
    let up = repo.resume_upload(&id).await.unwrap();
    assert_eq!(up.received().await.unwrap(), half as u64);
    assert!(matches!(up.write(0, b"x").await, Err(Error::Offset { .. })));
    up.write(half as u64, &data[half..]).await.unwrap();
    let f = up.finish().await.unwrap();
    assert_eq!(
        f.checksum,
        checksum(&data),
        "the checksum covers the bytes from before the restart"
    );
    assert_eq!(read_all(&repo, f.id).await, data);
    assert!(
        matches!(repo.resume_upload(&id).await, Err(Error::NotFound(_))),
        "session is gone"
    );
    assert!(matches!(
        repo.resume_upload("not-an-id").await,
        Err(Error::NotFound(_))
    ));
}

#[tokio::test]
async fn abort_cleans_up_bytes_and_session() {
    let env = env().await;
    let (up, id) = env
        .repo
        .start_upload(None, "gone.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"data").await.unwrap();
    up.abort().await.unwrap();
    assert!(matches!(
        env.repo.resume_upload(&id).await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(up.write(4, b"x").await, Err(Error::NotFound(_))));
    assert!(names(&env.repo, None).await.is_empty());
    let pending = std::fs::read_dir(env.dir.path().join("blobs/.fs-files/pending"))
        .unwrap()
        .count();
    assert_eq!(pending, 0);
    up.abort().await.unwrap(); // twice is fine
}

#[tokio::test]
async fn a_finished_upload_takes_no_more_bytes() {
    let env = env().await;
    let (up, _) = env
        .repo
        .start_upload(None, "done.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"x").await.unwrap();
    let file = up.finish().await.unwrap();
    assert!(up.write(1, b"y").await.is_err());
    // finishing again hands out the same file (never a second one)
    assert_eq!(up.finish().await.unwrap().id, file.id);
}

#[tokio::test]
async fn a_taken_name_leaves_the_upload_intact() {
    let _env = env().await;
    let e = _env.repo.clone();
    let (up, id) = e
        .start_upload(None, "same.txt", "text/plain")
        .await
        .unwrap();
    up.write(0, b"mine").await.unwrap();
    upload(&e, None, "same.txt", "text/plain", b"theirs").await;
    assert!(matches!(up.finish().await, Err(Error::Conflict(_))));
    assert_eq!(
        e.resume_upload(&id)
            .await
            .unwrap()
            .received()
            .await
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn the_media_type_is_detected_when_not_given() {
    let _env = env().await;
    let e = _env.repo.clone();
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
    assert_eq!(
        upload(&e, None, "p", "", &png).await.media_type,
        "image/png"
    );
    assert_eq!(
        upload(&e, None, "q", "application/octet-stream", b"plain")
            .await
            .media_type,
        "application/octet-stream"
    );
    assert_eq!(
        upload(&e, None, "r", "text/plain", &png).await.media_type,
        "text/plain",
        "given wins"
    );
}

#[tokio::test]
async fn an_empty_file_is_a_file() {
    let _env = env().await;
    let e = _env.repo.clone();
    let f = upload(&e, None, "empty", "text/plain", b"").await;
    assert_eq!((f.size, f.checksum.as_str()), (0, checksum(b"").as_str()));
    assert!(read_all(&e, f.id).await.is_empty());
}
