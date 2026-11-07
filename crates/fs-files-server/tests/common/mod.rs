//! Shared helpers of the integration tests: a real server on a real Unix socket, over a real
//! `FileRepo` (in-memory store, local folder in a temporary directory).

#![allow(dead_code)] // each test file uses a different subset

use std::path::PathBuf;
use std::sync::Arc;

use fs_files_core::{Backends, FileFeed, FileRepo, Files, LocalFolder, Transfer};
use fs_files_server::FilesServer;
use fs_interfaces::common_capnp::uuid as wire_uuid;
use fs_interfaces::files_capnp::{
    OnConflict, backends, entry, file, files, folders, search, transfer,
};
use fs_interfaces::rpc::{bind_socket, connect, read_envelope, read_uuid, serve, write_uuid};
use fs_store::{Envelope, Store, TursoStore};
use uuid::Uuid;

pub const ME: &str = "https://local.invalid/users/owner";

/// What a test sees of the program: the repository, the folder the files live in.
pub struct Fixture {
    pub repo: Arc<FileRepo>,
    pub root: PathBuf,
}

/// A real server in front of a real repository.
pub async fn with_server<F, Fut>(test: F)
where
    F: FnOnce(files::Client, Fixture) -> Fut,
    Fut: Future<Output = ()>,
{
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
    let root = dir.path().join("files");
    let backend = Arc::new(LocalFolder::open(&root).await.unwrap());
    let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
    let (files, transfer, backends, feed) =
        (repo.clone(), repo.clone(), repo.clone(), repo.clone());
    let fixture = Fixture { repo, root };
    with_provider(files, transfer, backends, feed, |c| test(c, fixture)).await;
}

/// A real server in front of this one repository.
pub async fn with_repo<F, Fut>(repo: Arc<FileRepo>, test: F)
where
    F: FnOnce(files::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let (a, b, c, d) = (repo.clone(), repo.clone(), repo.clone(), repo);
    with_provider(a, b, c, d, test).await;
}

/// A repository over a store file (with a key if `encrypted`, ADR 0024) and a local folder.
pub async fn repo_on_disk(dir: &std::path::Path, encrypted: bool) -> Arc<FileRepo> {
    use fs_store::{Cipher, EncryptionKey, OpenOptions, StaticKeyProvider};
    let mut opts = OpenOptions::default();
    if encrypted {
        let key = EncryptionKey {
            cipher: Cipher::Aes256Gcm,
            key: vec![7; 32],
        };
        opts.key_provider = Some(Arc::new(StaticKeyProvider(key)));
    }
    let store = TursoStore::open_with(dir.join("store.db"), opts)
        .await
        .unwrap();
    let store: Arc<dyn Store> = Arc::new(store);
    let backend = Arc::new(LocalFolder::open(dir.join("files")).await.unwrap());
    Arc::new(FileRepo::new(store, ME, "main", backend))
}

/// A real server in front of these providers.
pub async fn with_provider<F, Fut>(
    files: Arc<dyn Files>,
    transfer: Arc<dyn Transfer>,
    backends: Arc<dyn Backends>,
    feed: Arc<dyn FileFeed>,
    test: F,
) where
    F: FnOnce(files::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("files.sock");
            let root = FilesServer::new(files, transfer, backends, feed).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<files::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

/// The four capabilities behind the root.
pub struct Caps {
    pub folders: folders::Client,
    pub transfer: transfer::Client,
    pub search: search::Client,
    pub backends: backends::Client,
}

pub async fn caps(root: &files::Client) -> Caps {
    Caps {
        folders: root
            .folders_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_folders()
            .unwrap(),
        transfer: root
            .transfer_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_transfer()
            .unwrap(),
        search: root
            .search_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_search()
            .unwrap(),
        backends: root
            .backends_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_backends()
            .unwrap(),
    }
}

pub fn set_id(out: wire_uuid::Builder<'_>, id: Uuid) {
    write_uuid(out, Some(id));
}

/// A file as a consumer reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub id: Uuid,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub checksum: String,
    pub folder: Option<Uuid>,
    pub trashed: bool,
    pub envelope: Envelope,
}

pub fn read_file(f: file::Reader<'_>) -> Seen {
    let envelope = read_envelope(f.get_envelope().unwrap()).unwrap();
    Seen {
        id: envelope.id.as_uuid(),
        name: envelope.name.clone(),
        media_type: f.get_media_type().unwrap().to_string().unwrap(),
        size: f.get_size(),
        checksum: f.get_checksum().unwrap().to_string().unwrap(),
        folder: read_uuid(f.get_folder().unwrap()),
        trashed: f.get_trashed(),
        envelope,
    }
}

/// An entry as (name, is a folder, id).
pub fn read_entry(e: entry::Reader<'_>) -> (String, bool, Uuid) {
    match e.which().unwrap() {
        entry::Which::File(f) => {
            let f = read_file(f.unwrap());
            (f.name, false, f.id)
        }
        entry::Which::Folder(f) => {
            let env = read_envelope(f.unwrap().get_envelope().unwrap()).unwrap();
            (env.name, true, env.id.as_uuid())
        }
    }
}

/// Create a folder.
pub async fn mkdir(c: &Caps, parent: Option<Uuid>, name: &str) -> capnp::Result<Uuid> {
    let mut call = c.folders.create_folder_request();
    write_uuid(call.get().init_parent(), parent);
    call.get().set_name(name);
    let answer = call.send().promise.await?;
    let folder = answer.get()?.get_folder()?;
    Ok(read_envelope(folder.get_envelope()?)?.id.as_uuid())
}

/// Upload `data` in one chunk.
pub async fn put_file(
    c: &Caps,
    folder: Option<Uuid>,
    name: &str,
    media_type: &str,
    data: &[u8],
    on: OnConflict,
) -> capnp::Result<Seen> {
    let mut start = c.transfer.start_upload_request();
    write_uuid(start.get().init_folder(), folder);
    start.get().set_name(name);
    start.get().set_media_type(media_type);
    start.get().set_on_conflict(on);
    let started = start.send().promise.await?;
    let upload = started.get()?.get_upload()?;
    let mut write = upload.write_request();
    write.get().set_offset(0);
    write.get().set_data(data);
    write.send().promise.await?;
    let done = upload.finish_request().send().promise.await?;
    Ok(read_file(done.get()?.get_file()?))
}

/// A file with text in it, refused names are errors.
pub async fn text_file(c: &Caps, folder: Option<Uuid>, name: &str, text: &str) -> Seen {
    put_file(
        c,
        folder,
        name,
        "text/plain",
        text.as_bytes(),
        OnConflict::Refuse,
    )
    .await
    .unwrap()
}

/// Every entry of a folder, over all pages, as (name, is a folder, id).
pub async fn list_all(c: &Caps, folder: Option<Uuid>) -> Vec<(String, bool, Uuid)> {
    let (mut out, mut cursor) = (vec![], vec![]);
    loop {
        let mut call = c.folders.list_request();
        write_uuid(call.get().init_folder(), folder);
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.unwrap();
        let page = answer.get().unwrap().get_page().unwrap();
        out.extend(page.get_items().unwrap().iter().map(read_entry));
        cursor = page.get_next().unwrap().get_opaque().unwrap().to_vec();
        if cursor.is_empty() {
            return out;
        }
    }
}

pub fn names(entries: &[(String, bool, Uuid)]) -> Vec<&str> {
    entries.iter().map(|e| e.0.as_str()).collect()
}

/// All bytes of a file, read through its `Blob` in pieces of `piece`.
pub async fn download(c: &Caps, id: Uuid, piece: u32) -> capnp::Result<Vec<u8>> {
    let mut open = c.transfer.open_request();
    set_id(open.get().init_id(), id);
    let blob = open.send().promise.await?.get()?.get_blob()?;
    let mut out = vec![];
    loop {
        let mut read = blob.read_request();
        read.get().set_offset(out.len() as u64);
        read.get().set_length(piece);
        let answer = read.send().promise.await?;
        let data = answer.get()?.get_data()?;
        if data.is_empty() {
            return Ok(out);
        }
        out.extend_from_slice(data);
    }
}

/// The problem an error carries: its type and status must be exactly these.
pub fn is(error: &capnp::Error, problem_type: &str, status: u16) {
    let p = fs_interfaces::rpc::Problem::from_error(error).expect("a typed problem");
    assert_eq!(
        (p.problem_type.as_str(), p.status),
        (problem_type, status),
        "{}",
        p.detail
    );
}

/// An unset id reads as `None`.
pub fn read_uuid_of(r: wire_uuid::Reader<'_>) -> Option<Uuid> {
    read_uuid(r)
}

/// An id that exists nowhere.
pub fn unknown_id() -> Uuid {
    Envelope::new("Document", "x").id.as_uuid()
}

/// The checksum the contract names for these bytes.
pub fn checksum(data: &[u8]) -> String {
    fs_files_core::format_checksum(&blake3::hash(data))
}

/// Start an upload; the capability and its id.
pub async fn start(
    c: &Caps,
    folder: Option<Uuid>,
    name: &str,
) -> capnp::Result<(fs_interfaces::files_capnp::upload::Client, String)> {
    let mut call = c.transfer.start_upload_request();
    write_uuid(call.get().init_folder(), folder);
    call.get().set_name(name);
    let answer = call.send().promise.await?;
    let r = answer.get()?;
    Ok((r.get_upload()?, r.get_id()?.to_string()?))
}

pub async fn received(upload: &fs_interfaces::files_capnp::upload::Client) -> u64 {
    let answer = upload.received_request().send().promise.await.unwrap();
    answer.get().unwrap().get_bytes()
}
