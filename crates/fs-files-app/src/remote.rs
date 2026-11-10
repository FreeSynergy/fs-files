//! `FilesRemote`: the files server behind a socket.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection and every call is
//! a job handed to it; the caller waits on a plain channel (the window thread is not inside a runtime). The `Backend`
//! of each data type for the generated frontend is implemented on it in the `backend_*` modules.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use fs_admin_frontend::BackendError;
use fs_interfaces::files_capnp::{backends, files, folders, search, transfer};
use fs_interfaces::rpc::{Problem, connect};
use fs_types::{RecordIssue, RecordProblem, TypeProblem};

/// The clients a job works with.
pub(crate) struct Clients {
    pub(crate) files: files::Client,
    pub(crate) folders: folders::Client,
    pub(crate) search: search::Client,
    pub(crate) transfer: transfer::Client,
    pub(crate) backends: backends::Client,
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

    pub(crate) fn run<T, F, Fut>(&self, make: F) -> Result<T, BackendError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, BackendError>> + 'static,
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

fn gone() -> BackendError {
    BackendError::Unavailable("the files server went away".into())
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
    let transfer = root
        .transfer_request()
        .send()
        .promise
        .await?
        .get()?
        .get_transfer()?;
    let backends = root
        .backends_request()
        .send()
        .promise
        .await?
        .get()?
        .get_backends()?;
    Ok(Clients {
        files: root.clone(),
        folders,
        search,
        transfer,
        backends,
    })
}

/// The refusal for a command the core does not have; the frontend shows its control locked and never sends it.
pub(crate) fn unsupported(name: &str) -> BackendError {
    BackendError::Unavailable(format!("unsupported:{name}"))
}

fn finding(field: &str, problem: TypeProblem) -> BackendError {
    BackendError::Rejected(vec![RecordIssue {
        field: field.to_owned(),
        index: 0,
        problem: RecordProblem::Value(problem),
    }])
}

/// What the server said, in the frontend's terms; the server's own words are kept for what is not a finding.
///
/// A name that is taken has no code in `fs-types` (`TypeProblem`); it is reported as `NotInList` on the field `name`
/// (reported to the strategy). An invalid name is `empty`, `too-long` or `character` by its reason.
pub(crate) fn problem(error: &capnp::Error) -> BackendError {
    let Some(p) = Problem::from_error(error) else {
        return BackendError::Unavailable(error.to_string());
    };
    if p.is_unsupported() {
        // the detail of "not possible" is `capability: reason`; the name is the capability
        let name = p.detail.split(':').next().unwrap_or_default().trim();
        return BackendError::Unavailable(format!("unsupported:{name}"));
    }
    match p.problem_type.rsplit('/').next().unwrap_or_default() {
        "not-found" => BackendError::NotFound,
        "name-conflict" => finding("name", TypeProblem::NotInList),
        "invalid-name" => {
            let issue = p.ext("issue").unwrap_or(&p.detail).to_lowercase();
            finding(
                "name",
                if issue.contains("empty") {
                    TypeProblem::Empty
                } else if issue.contains("long") {
                    TypeProblem::TooLong
                } else {
                    TypeProblem::Character
                },
            )
        }
        _ => BackendError::Unavailable(p.detail),
    }
}
