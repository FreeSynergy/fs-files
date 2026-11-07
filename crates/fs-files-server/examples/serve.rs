//! Serve a `FileRepo` over a Unix socket and use it as a consumer would: make a folder, upload a
//! file in two chunks, list the folder, read the file back.
//!
//! Pattern: Facade — `FilesServer` is the one entry point; the client side is the generated one.
//! Run with `cargo run -p fs-files-server --example serve`.

use std::sync::Arc;

use fs_files_core::{FileRepo, LocalFolder};
use fs_files_server::FilesServer;
use fs_interfaces::files_capnp::files;
use fs_interfaces::rpc::{bind_socket, connect, read_envelope, serve};
use fs_objects::{Store, TursoStore};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local = tokio::task::LocalSet::new();
    local.run_until(run()).await
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let backend = Arc::new(LocalFolder::open(dir.path().join("files")).await?);
    let repo = Arc::new(FileRepo::new(
        store,
        "https://local.invalid/users/owner",
        "main",
        backend,
    ));
    let root = FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo).into_client();

    let socket = dir.path().join("files.sock");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let serving =
        tokio::task::spawn_local(serve(bind_socket(&socket).await?, root.client, async {
            let _ = stopped.await;
        }));

    let connection = connect::<files::Client>(&socket).await?;
    let folders = connection.client.folders_request().send().promise.await?;
    let folders = folders.get()?.get_folders()?;
    let transfer = connection.client.transfer_request().send().promise.await?;
    let transfer = transfer.get()?.get_transfer()?;

    let mut make = folders.create_folder_request();
    make.get().init_parent();
    make.get().set_name("Docs");
    let made = make.send().promise.await?;
    let docs = read_envelope(made.get()?.get_folder()?.get_envelope()?)?
        .id
        .as_uuid();

    let mut start = transfer.start_upload_request();
    fs_interfaces::rpc::write_uuid(start.get().init_folder(), Some(docs));
    start.get().set_name("hello.txt");
    let upload = start.send().promise.await?.get()?.get_upload()?;
    for (offset, part) in [(0, "hello, "), (7, "files")] {
        let mut write = upload.write_request();
        write.get().set_offset(offset);
        write.get().set_data(part.as_bytes());
        write.send().promise.await?;
    }
    let done = upload.finish_request().send().promise.await?;
    let file = done.get()?.get_file()?;
    println!(
        "uploaded {} bytes as {}",
        file.get_size(),
        read_envelope(file.get_envelope()?)?.name
    );

    let mut list = folders.list_request();
    fs_interfaces::rpc::write_uuid(list.get().init_folder(), Some(docs));
    let listed = list.send().promise.await?;
    println!(
        "{} entry in Docs",
        listed.get()?.get_page()?.get_items()?.len()
    );

    println!(
        "on the disk: {:?}",
        std::fs::read_to_string(dir.path().join("files/Docs/hello.txt"))?
    );
    drop(connection);
    let _ = stop.send(());
    serving.await??;
    Ok(())
}
