//! Files over the wire: resolve, rename, move, copy; values compared, the disk checked.
#![allow(clippy::assert_is_empty)] // tests: plain `is_empty()` asserts read clearer than typed empty comparisons

mod common;

use common::{caps, mkdir, read_file, set_id, text_file, with_server};
use fs_interfaces::files_capnp::OnConflict;
use fs_interfaces::rpc::write_uuid;

const BLAKE_OF_HELLO: &str =
    "blake3:ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f";

#[tokio::test(flavor = "current_thread")]
async fn a_file_arrives_with_what_the_core_knows_of_it() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let dir = mkdir(&c, None, "Docs").await.unwrap();
        let sent = text_file(&c, Some(dir), "hello.txt", "hello").await;
        assert_eq!(sent.checksum, BLAKE_OF_HELLO);
        assert_eq!((sent.size, sent.media_type.as_str()), (5, "text/plain"));
        assert_eq!((sent.folder, sent.trashed), (Some(dir), false));
        assert_eq!(sent.envelope.object_type, "Document");
        assert_eq!(sent.envelope.context.as_deref(), Some("files"));
        assert_eq!(sent.envelope.attributed_to.as_deref(), Some(common::ME));
        assert_eq!(
            std::fs::read(fx.root.join("Docs/hello.txt")).unwrap(),
            b"hello"
        );

        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), sent.id);
        let answer = get.send().promise.await.unwrap();
        assert!(answer.get().unwrap().get_found());
        let got = read_file(answer.get().unwrap().get_file().unwrap());
        assert_eq!(got, sent, "what the upload answered is what get answers");

        let mut missing = c.folders.get_request();
        set_id(missing.get().init_id(), common::unknown_id());
        let answer = missing.send().promise.await.unwrap();
        assert!(!answer.get().unwrap().get_found());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn rename_move_and_copy_change_the_record_and_the_disk() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let dir = mkdir(&c, None, "Archive").await.unwrap();
        let file = text_file(&c, None, "a.txt", "hello").await;

        let mut rename = c.folders.rename_file_request();
        set_id(rename.get().init_id(), file.id);
        rename.get().set_name("b.txt");
        rename.get().set_on_conflict(OnConflict::Refuse);
        let answer = rename.send().promise.await.unwrap();
        let renamed = read_file(answer.get().unwrap().get_file().unwrap());
        assert_eq!((renamed.name.as_str(), renamed.id), ("b.txt", file.id));
        assert!(renamed.envelope.updated >= file.envelope.updated);
        assert!(fx.root.join("b.txt").is_file() && !fx.root.join("a.txt").exists());

        let mut copy = c.folders.copy_file_request();
        set_id(copy.get().init_id(), file.id);
        write_uuid(copy.get().init_folder(), Some(dir));
        copy.get().set_on_conflict(OnConflict::KeepBoth);
        let answer = copy.send().promise.await.unwrap();
        let copied = read_file(answer.get().unwrap().get_file().unwrap());
        assert_ne!(copied.id, file.id);
        assert_eq!((copied.name.as_str(), copied.folder), ("b.txt", Some(dir)));
        assert_eq!(copied.checksum, file.checksum);
        assert_eq!(
            std::fs::read(fx.root.join("Archive/b.txt")).unwrap(),
            b"hello"
        );

        let mut moved = c.folders.move_file_request();
        set_id(moved.get().init_id(), file.id);
        write_uuid(moved.get().init_folder(), Some(dir));
        moved.get().set_on_conflict(OnConflict::KeepBoth);
        let answer = moved.send().promise.await.unwrap();
        let moved = read_file(answer.get().unwrap().get_file().unwrap());
        assert_eq!(
            (moved.name.as_str(), moved.folder),
            ("b (2).txt", Some(dir))
        );
        assert!(fx.root.join("Archive/b (2).txt").is_file() && !fx.root.join("b.txt").exists());
    })
    .await;
}

async fn classify(c: &common::Caps, id: uuid::Uuid, category: &str) -> capnp::Result<common::Seen> {
    let mut call = c.folders.classify_request();
    set_id(call.get().init_id(), id);
    call.get().init_categories(1).set(0, category);
    let answer = call.send().promise.await?;
    Ok(read_file(answer.get()?.get_file()?))
}

#[tokio::test(flavor = "current_thread")]
async fn classify_goes_to_the_envelope_of_an_encrypted_store() {
    let dir = tempfile::tempdir().unwrap();
    let repo = common::repo_on_disk(dir.path(), true).await;
    common::with_repo(repo, |root| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "tax.txt", "x").await;
        let marked = classify(&c, file.id, "dpv:Financial").await.unwrap();
        assert_eq!(marked.envelope.sensitive, ["dpv:Financial"]);
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_store_without_a_key_refuses_and_the_file_stays_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let repo = common::repo_on_disk(dir.path(), false).await;
    common::with_repo(repo, |root| async move {
        let c = caps(&root).await;
        let file = text_file(&c, None, "tax.txt", "x").await;
        let error = classify(&c, file.id, "dpv:Financial").await.err().unwrap();
        let kind = "https://freesynergy.net/problems/files/encryption-required";
        common::is(&error, kind, 409);
        let mut get = c.folders.get_request();
        set_id(get.get().init_id(), file.id);
        let answer = get.send().promise.await.unwrap();
        let got = read_file(answer.get().unwrap().get_file().unwrap());
        assert!(got.envelope.sensitive.is_empty());
    })
    .await;
}
