//! Bytes in and out: chunks, resuming with a new handle, the list of unfinished uploads, ranges.

mod common;

use common::Served;
use fs_files_core::{Error, Files, OnConflict, Transfer};
use fs_store::ObjectId;

const MIB: usize = 1 << 20;

fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| u8::try_from(i % 251).unwrap()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_goes_up_in_chunks_and_lands_on_the_disk_whole() {
    let served = Served::start().await;
    let files = served.client();
    let data = bytes(3 * MIB + 17);
    let (up, id) = files
        .start_upload_as(None, "big.bin", "", OnConflict::Refuse)
        .await
        .unwrap();
    assert!(!id.is_empty());
    let mut at = 0;
    for chunk in data.chunks(MIB) {
        up.write(at as u64, chunk).await.unwrap();
        at += chunk.len();
        assert_eq!(up.received().await.unwrap(), at as u64);
    }
    let file = up.finish().await.unwrap();
    assert_eq!(
        (file.name.as_str(), file.size),
        ("big.bin", data.len() as u64)
    );
    assert_eq!(std::fs::read(served.root.join("big.bin")).unwrap(), data);
    let on_server = served.repo.get(file.id).await.unwrap().unwrap();
    assert_eq!(file.checksum, on_server.checksum);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_goes_on_with_a_new_handle_and_a_wrong_offset_writes_nothing() {
    let served = Served::start().await;
    let files = served.client();
    let data = bytes(2 * MIB);
    let (up, id) = files
        .start_upload_as(None, "r.bin", "", OnConflict::Refuse)
        .await
        .unwrap();
    up.write(0, &data[..MIB]).await.unwrap();
    drop(up);
    let up = files.resume_upload(&id).await.unwrap();
    let held = up.received().await.unwrap();
    assert_eq!(held, MIB as u64);
    let wrong = up.write(5, &data[MIB..]).await.unwrap_err();
    assert!(
        matches!(wrong, Error::Offset { expected, got } if expected == held && got == 5),
        "{wrong:?}"
    );
    up.write(held, &data[MIB..]).await.unwrap();
    let file = up.finish().await.unwrap();
    assert_eq!(std::fs::read(served.root.join("r.bin")).unwrap(), data);
    assert_eq!(file.size, data.len() as u64);
}

#[tokio::test(flavor = "multi_thread")]
async fn unfinished_uploads_are_listed_over_every_page_and_can_be_dropped() {
    let served = Served::start().await;
    let files = served.client();
    for i in 0..105 {
        files
            .start_upload_as(None, &format!("u{i:03}.bin"), "", OnConflict::Refuse)
            .await
            .unwrap();
    }
    let listed = files.pending_uploads().await.unwrap();
    let held = served.repo.pending_uploads().await.unwrap();
    assert_eq!(listed.len(), 105);
    assert_eq!(
        listed
            .iter()
            .map(|u| (&u.id, &u.name, u.received))
            .collect::<Vec<_>>(),
        held.iter()
            .map(|u| (&u.id, &u.name, u.received))
            .collect::<Vec<_>>()
    );
    let up = files.resume_upload(&listed[0].id).await.unwrap();
    up.abort().await.unwrap();
    assert_eq!(files.pending_uploads().await.unwrap().len(), 104);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_taken_name_stops_an_upload_at_the_start_and_keep_both_gets_a_free_one() {
    let served = Served::start().await;
    let files = served.client();
    served.file(None, "a.txt", b"one").await;
    let err = files
        .start_upload_as(None, "a.txt", "", OnConflict::Refuse)
        .await
        .map(|_| ())
        .unwrap_err();
    assert!(err.problem().is_some(), "{err:?}");
    let (up, _) = files
        .start_upload_as(None, "a.txt", "", OnConflict::KeepBoth)
        .await
        .unwrap();
    up.write(0, b"two").await.unwrap();
    assert_eq!(up.finish().await.unwrap().name, "a (2).txt");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_is_read_in_ranges_and_a_missing_one_is_not_found() {
    let served = Served::start().await;
    let files = served.client();
    let data = bytes(MIB + 5);
    let made = served.file(None, "x.bin", &data).await;
    let blob = files.open(made.id).await.unwrap();
    let (size, _) = blob.size().await.unwrap();
    assert_eq!(size, data.len() as u64);
    let mut got = Vec::new();
    while got.len() < data.len() {
        let part = blob.read(got.len() as u64, 300_000).await.unwrap();
        assert!(!part.is_empty());
        got.extend(part);
    }
    assert_eq!(got, data);
    assert!(
        blob.read(size, 10).await.unwrap().is_empty(),
        "past the end is empty"
    );
    let missing = files.open(ObjectId::new()).await.map(|_| ()).unwrap_err();
    assert!(matches!(missing, Error::NotFound(_)), "{missing:?}");
}
