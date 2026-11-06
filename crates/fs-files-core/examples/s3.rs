//! Add an S3 backend (RustFS or any S3 store) at runtime and upload a file to it.
//!
//! FS_FILES_S3_ENDPOINT=http://127.0.0.1:9000 FS_FILES_S3_BUCKET=files \
//! FS_FILES_S3_ACCESS_KEY=... FS_FILES_S3_SECRET_KEY=... cargo run -p fs-files-core --example s3

use std::sync::Arc;

use fs_files_core::{Backends, FileRepo, Files, LocalFolder, Transfer};
use fs_store::{Store, TursoStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let var = |n: &str| std::env::var(format!("FS_FILES_S3_{n}"));
    let dir = std::env::temp_dir().join("fs-files-s3-example");
    let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await?);
    let local = Arc::new(LocalFolder::open(&dir).await?);
    let files = FileRepo::new(store, "https://local.invalid/users/owner", "local", local);

    // The configuration is checked (a probe blob is written, read and deleted) before it is swapped in.
    let config = serde_json::json!({
        "id": "cloud", "kind": "s3", "active": true,
        "endpoint": var("ENDPOINT")?, "bucket": var("BUCKET")?,
        "access_key": var("ACCESS_KEY")?, "secret_key": var("SECRET_KEY")?,
    });
    println!("added: {:?}", files.add_backend(&config.to_string()).await?);

    let (upload, _) = files.start_upload(None, "hello.txt", "text/plain").await?;
    upload.write(0, b"hello from S3").await?;
    let file = upload.finish().await?;
    println!(
        "{} is on backend {}",
        file.name,
        files.backend_of(file.id).await?
    );
    files.trash(file.id).await?;
    files.purge(file.id).await?;
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
