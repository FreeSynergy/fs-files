//! `fs-files-server [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN]`; the folder the
//! files live in is `FS_FILES_ROOT` (default `files` below the data directory). Files are kept under
//! their real names there.
//!
//! Pattern: none (thin binary) — reads its locations, opens store and local folder, serves until a signal.

use std::path::PathBuf;
use std::sync::Arc;

use fs_files_core::{FileRepo, LocalFolder};
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal};
use fs_store::{Store, TursoStore};

const ROOT: &str = "FS_FILES_ROOT";
/// The id files record for the backend this program starts with.
const BACKEND_ID: &str = "main";

fn main() {
    let options = ServerOptions::from_env_or_exit("files");
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    let served = run_until_signal(&socket, || async move {
        let data = prepare_data_dir(&data)?;
        let root = std::env::var_os(ROOT)
            .filter(|v| !v.is_empty())
            .map_or_else(|| data.join("files"), PathBuf::from);
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let local = LocalFolder::open(root)
            .await
            .map_err(std::io::Error::other)?;
        let mut repo = FileRepo::new(store, actor, BACKEND_ID, Arc::new(local));
        if let Some(instance) = instance {
            repo = repo.with_instance(instance);
        }
        let repo = Arc::new(repo);
        Ok(
            FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo)
                .into_client()
                .client,
        )
    });
    if let Err(e) = served {
        eprintln!("fs-files-server: {e}");
        std::process::exit(1);
    }
}
