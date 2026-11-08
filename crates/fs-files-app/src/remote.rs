//! `FilesRemote`: the files server behind a socket.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection
//! and every call is a job handed to it; the caller waits on a plain channel (the window thread is not
//! inside a runtime). The `FileSource` of the workbench is implemented on it in `source.rs`.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use fs_components_workbenches::file_manager::{FileError, FileProblem};
use fs_interfaces::files_capnp::{files, folders, search};
use fs_interfaces::rpc::{Problem, connect};

/// The clients a job works with.
pub(crate) struct Clients {
    pub(crate) folders: folders::Client,
    pub(crate) search: search::Client,
}

type Job = Box<dyn FnOnce(Rc<Clients>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// The files server on a socket.
pub struct FilesRemote {
    jobs: tokio::sync::mpsc::UnboundedSender<Job>,
}

impl FilesRemote {
    /// Connect to the server on `socket`.
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect(socket: &Path) -> Result<Self, String> {
        let (jobs, inbox) = tokio::sync::mpsc::unbounded_channel();
        let (ready, started) = mpsc::channel();
        let socket = socket.to_path_buf();
        thread::Builder::new()
            .name("files-rpc".into())
            .spawn(move || worker(&socket, inbox, &ready))
            .map_err(|e| e.to_string())?;
        started.recv().map_err(|e| e.to_string())??;
        Ok(Self { jobs })
    }

    pub(crate) fn run<T, F, Fut>(&self, make: F) -> Result<T, FileError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, FileError>> + 'static,
    {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |clients| {
            Box::pin(async move {
                let _ = reply.send(make(clients).await);
            })
        });
        self.jobs.send(job).map_err(|_| gone())?;
        answer.recv().map_err(|_| gone())?
    }
}

fn gone() -> FileError {
    FileError::failed("the files server went away")
}

fn worker(
    socket: &Path,
    mut inbox: tokio::sync::mpsc::UnboundedReceiver<Job>,
    ready: &mpsc::Sender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    tokio::task::LocalSet::new().block_on(&runtime, async move {
        let connection = match connect::<files::Client>(socket).await {
            Ok(c) => c,
            Err(e) => {
                let _ = ready.send(Err(format!(
                    "no files server on {}: {e}; start it with `fs-files serve`",
                    socket.display()
                )));
                return;
            }
        };
        let clients = match open(&connection.client).await {
            Ok(c) => Rc::new(c),
            Err(e) => {
                let _ = ready.send(Err(e.to_string()));
                return;
            }
        };
        let _ = ready.send(Ok(()));
        while let Some(job) = inbox.recv().await {
            tokio::task::spawn_local(job(Rc::clone(&clients)));
        }
    });
}

async fn open(root: &files::Client) -> capnp::Result<Clients> {
    let folders = root
        .folders_request()
        .send()
        .promise
        .await?
        .get()?
        .get_folders()?;
    let search = root
        .search_request()
        .send()
        .promise
        .await?
        .get()?
        .get_search()?;
    Ok(Clients { folders, search })
}

/// What the server said, in the workbench's terms; the server's own words are kept.
pub(crate) fn problem(error: &capnp::Error) -> FileError {
    let Some(p) = Problem::from_error(error) else {
        return FileError::failed(error.to_string());
    };
    if p.is_unsupported() {
        return FileError::unsupported(p.detail);
    }
    let kind = p.problem_type.rsplit('/').next().unwrap_or_default();
    let which = match kind {
        "name-conflict" => FileProblem::NameConflict,
        "invalid-name" => FileProblem::InvalidName,
        "changed-outside" => FileProblem::ChangedOutside,
        "not-found" => FileProblem::NotFound,
        "encryption-required" => FileProblem::MarkedNeedsRelease,
        _ => FileProblem::Failed,
    };
    FileError::new(which, p.detail)
}
