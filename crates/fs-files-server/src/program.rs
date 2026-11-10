//! `serve`: the server as a command of the one program (ADR 0063): `fs-files serve [--socket PATH]
//! [--data DIR] [--actor IRI] [--instance DOMAIN]`; the folder the files live in is `FS_FILES_ROOT`
//! (default `files` below the data directory). Files are kept under their real names there.
//!
//! Pattern: Facade — reads its locations, opens store and local folder, serves until a signal.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use fs_files_core::{FileRepo, LocalFolder, RESERVED_NAME};
use fs_interfaces::rpc::{ServerOptions, prepare_data_dir, run_until_signal, stop_self};
use fs_objects::{Store, TursoStore};

use crate::{FilesServer, check_places};

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

/// The help text of `fs-files serve`.
pub const HELP: &str = "fs-files serve [--socket PATH] [--data DIR] [--actor IRI] [--instance DOMAIN] [--lease FILE]\nThe files live in $FS_FILES_ROOT (default: files below the data folder).\n--lease FILE: end when no window holds this lock any more (started by a window, not by hand)\nDefaults: $XDG_RUNTIME_DIR/freesynergy/files.sock, $XDG_DATA_HOME/freesynergy/files, the local owner";

/// Why `serve` ended without serving.
#[derive(Debug, PartialEq, Eq)]
pub enum ServeError {
    /// The words or the settings were wrong; the caller should exit with 2.
    Usage(String),
    /// The server could not start or stopped with an error; exit with 1.
    Failed(String),
}

/// Serve `files.capnp` until a signal arrives. `args` are the words after `serve`.
///
/// # Errors
/// [`ServeError::Usage`] for unknown or incomplete words or a bad `FS_FILES_ROOT`,
/// [`ServeError::Failed`] for everything else.
pub fn serve(args: impl IntoIterator<Item = OsString>) -> Result<(), ServeError> {
    let (lease, args) =
        crate::lease::take_lease(args.into_iter().collect()).map_err(ServeError::Usage)?;
    let Some(options) = ServerOptions::parse("files", args).map_err(ServeError::Usage)? else {
        println!("{HELP}");
        return Ok(());
    };
    let root = root_of_env().map_err(ServeError::Usage)?;
    let ServerOptions {
        socket,
        data,
        actor,
        instance,
    } = options;
    if let Some(lease) = lease {
        // The window that started this server holds the lease; when none does, end as a signal would.
        crate::lease::watch(lease, stop_self);
    }
    run_until_signal(&socket, || async move {
        let data = prepare_data_dir(&data)?;
        let root = root.unwrap_or_else(|| data.join("files"));
        check_places(&root, &data).map_err(std::io::Error::other)?;
        let store = TursoStore::open(data.join("objects.db"))
            .await
            .map_err(std::io::Error::other)?;
        let store: Arc<dyn Store> = Arc::new(store);
        let local = LocalFolder::open(&root)
            .await
            .map_err(std::io::Error::other)?;
        // `backend.add` comes over the socket: its local folders live in the program's own folder inside
        // the root, which a rescan never lists, so no backend is part of another.
        let mut repo = FileRepo::new(store, actor, BACKEND_ID, Arc::new(local))
            .with_local_base(root.join(RESERVED_NAME).join("backends"));
        if let Some(instance) = instance {
            repo = repo.with_instance(instance);
        }
        let repo = Arc::new(repo);
        Ok(
            FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo)
                .into_client()
                .client,
        )
    })
    .map_err(|e| ServeError::Failed(e.to_string()))
}
