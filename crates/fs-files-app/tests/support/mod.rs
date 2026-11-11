//! Test support: a real server on its own thread and socket, and the helpers the backend tests share.
//!
//! Pattern: none (tests).
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use fs_admin_frontend::{Backend, Values};
use fs_files_app::FilesRemote;
use fs_files_core::{FileRepo, LocalFolder};
use fs_files_server::FilesServer;
use fs_interfaces::rpc::{ServerIdentity, bind_socket, run_until_signal_as, serve};
use fs_objects::{Store, TursoStore};

const ME: &str = "https://local.invalid/users/owner";

fn start_server(socket: &Path, root: &Path) {
    let (socket, root) = (socket.to_path_buf(), root.to_path_buf());
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, async move {
            let store: Arc<dyn Store> = Arc::new(TursoStore::open_in_memory().await.unwrap());
            let backend = Arc::new(LocalFolder::open(&root).await.unwrap());
            let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
            let server = FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo);
            let bound = bind_socket(&socket).await.unwrap();
            up.send(()).unwrap();
            serve(bound, server.into_client().client, std::future::pending())
                .await
                .ok();
        });
    });
    started.recv().unwrap();
}

pub fn v(pairs: &[(&str, &str)]) -> Values {
    pairs
        .iter()
        .map(|(k, x)| ((*k).to_owned(), vec![(*x).to_owned()]))
        .collect()
}

pub fn url(p: &Path) -> String {
    // Windows writes the path after three slashes, with the drive letter first: `file:///C:/Users/me/a.txt`.
    #[cfg(windows)]
    return format!("file:///{}", p.display().to_string().replace('\\', "/"));
    #[cfg(unix)]
    return format!("file://{}", p.display());
}

pub struct World {
    pub _dir: tempfile::TempDir,
    pub dir: std::path::PathBuf,
    pub remote: Arc<FilesRemote>,
}

pub fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let (socket, root) = (dir.path().join("files.sock"), dir.path().join("files"));
    start_server(&socket, &root);
    let remote = Arc::new(FilesRemote::connect(&socket).unwrap());
    World {
        dir: dir.path().to_path_buf(),
        _dir: dir,
        remote,
    }
}

pub fn names(b: &dyn Backend, query: &Values) -> Vec<String> {
    let mut n: Vec<String> = b
        .list(query)
        .unwrap()
        .iter()
        .map(|r| r.values["name"][0].clone())
        .collect();
    n.sort();
    n
}

/// A real server on its own thread that says it is `fs-files` of `version` (`Control.version`), the way `fs-files serve`
/// does; it ends with the process. Returns when it listens.
pub fn serve_as(socket: &Path, root: &Path, version: &str) {
    let (socket, root, version) = (socket.to_path_buf(), root.to_path_buf(), version.to_owned());
    let (up, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = run_until_signal_as(
            &socket,
            ServerIdentity::new("fs-files", version),
            || async move {
                let store: Arc<dyn Store> = Arc::new(
                    TursoStore::open_in_memory()
                        .await
                        .map_err(std::io::Error::other)?,
                );
                let backend = Arc::new(
                    LocalFolder::open(&root)
                        .await
                        .map_err(std::io::Error::other)?,
                );
                let repo = Arc::new(FileRepo::new(store, ME, "main", backend));
                let server = FilesServer::new(repo.clone(), repo.clone(), repo.clone(), repo);
                up.send(()).ok();
                Ok(server.into_client().client)
            },
        );
    });
    started.recv().expect("the server started");
}
