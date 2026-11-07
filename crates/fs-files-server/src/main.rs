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

/// The folder the files live in. A relative path would mean another folder for every working directory.
fn root_of_env() -> Result<Option<PathBuf>, String> {
    let Some(root) = std::env::var_os(ROOT).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let root = PathBuf::from(root);
    if root.is_absolute() {
        Ok(Some(root))
    } else {
        Err(format!(
            "{ROOT} must be an absolute path, not {}",
            root.display()
        ))
    }
}

fn main() {
    let options = ServerOptions::from_env_or_exit("files");
    let root = root_of_env().unwrap_or_else(|problem| {
        eprintln!("fs-files-server: {problem}");
        std::process::exit(2);
    });
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    let served = run_until_signal(&socket, || async move {
        let data = prepare_data_dir(&data)?;
        let root = root.unwrap_or_else(|| data.join("files"));
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
