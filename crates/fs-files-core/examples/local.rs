//! Upload a file in chunks into a local folder, list it, read a range, trash and purge it.
//!
//! cargo run -p fs-files-core --example local

use std::sync::Arc;

use fs_files_core::{Entry, FileRepo, Files, LocalFolder, Transfer};
use fs_store::{Store, TursoStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join("fs-files-example");
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let backend = Arc::new(LocalFolder::open(&dir).await?);
    // Program against the traits; `FileRepo` is the store-backed implementation.
    let files = Arc::new(FileRepo::new(
        store,
        "https://local.invalid/users/owner",
        "main",
        backend,
    ));

    let docs = files.create_folder(None, "Documents").await?;
    let (upload, id) = files.start_upload(Some(docs.id), "hello.txt", "").await?;
    upload.write(0, b"hello, ").await?;
    println!(
        "started upload {id}, {} bytes so far",
        upload.received().await?
    );
    upload.write(upload.received().await?, b"files").await?; // resume where it ended
    let file = upload.finish().await?;
    println!(
        "{} ({}, {} bytes, {})",
        file.name, file.media_type, file.size, file.checksum
    );

    for entry in files.list(Some(docs.id), None).await?.items {
        if let Entry::File(f) = entry {
            println!("in Documents: {}", f.name);
        }
    }
    let blob = files.open(file.id).await?;
    println!(
        "bytes 7..12: {}",
        String::from_utf8_lossy(&blob.read(7, 5).await?)
    );

    files.trash(file.id).await?;
    files.purge(file.id).await?;
    files.delete_folder(docs.id).await?;
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
