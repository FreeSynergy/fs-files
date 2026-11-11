//! `FilesRemote`: the files server behind a socket.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection and every call is
//! a job handed to it; the caller waits on a plain channel (the window thread is not inside a runtime). The `Backend`
//! of each data type for the generated frontend is implemented on it in the `backend_*` modules.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;

use fs_admin_frontend::BackendError;
use fs_interfaces::files_capnp::{backends, files, folders, search, transfer};
use fs_interfaces::rpc::{Connection, Expected, Problem, ServerStarter, connect, connect_matching};
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

/// How the connection is made: plain (a server that is there, as the tests have it) or with the version handshake.
enum Dial {
    Plain(PathBuf),
    Matching {
        socket: PathBuf,
        expected: Expected,
        starter: Arc<dyn ServerStarter + Send + Sync>,
    },
}

/// The state of the connection: a worker that answers, or why there is none (the window shows it, "Try again").
enum Link {
    Up(tokio::sync::mpsc::UnboundedSender<Job>),
    Down(BackendError),
}

/// The files server behind a socket.
pub struct FilesRemote {
    dial: Dial,
    link: Mutex<Link>,
}

impl FilesRemote {
    /// Connect to the server that is on `socket`, with no version check and no start (what the tests need).
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect(socket: &Path) -> Result<Self, String> {
        let dial = Dial::Plain(socket.to_path_buf());
        match dial.open() {
            Ok(jobs) => Ok(Self {
                dial,
                link: Mutex::new(Link::Up(jobs)),
            }),
            Err(BackendError::Handshake(text) | BackendError::Unavailable(text)) => Err(text),
            Err(other) => Err(format!("{other:?}")),
        }
    }

    /// Reach the server on `socket` that fits this program: the version handshake of fs-interfaces. One that runs and
    /// differs is replaced, none running is started by `starter`. This never fails: when no fitting server can be
    /// reached, the remote keeps the reason, every call answers [`BackendError::Handshake`] with it, and
    /// [`FilesRemote::reconnect`] (the "Try again" of the window) asks again. The window opens in any case.
    pub fn matching(socket: &Path, starter: Arc<dyn ServerStarter + Send + Sync>) -> Self {
        let dial = Dial::Matching {
            socket: socket.to_path_buf(),
            expected: Expected::new(env!("CARGO_PKG_VERSION")),
            starter,
        };
        let link = match dial.open() {
            Ok(jobs) => Link::Up(jobs),
            Err(e) => Link::Down(e),
        };
        Self {
            dial,
            link: Mutex::new(link),
        }
    }

    /// Connect again (after a failed handshake, or a server that went away) and keep the new connection.
    ///
    /// # Errors
    /// [`BackendError::Handshake`] (again) or [`BackendError::Unavailable`].
    pub fn reconnect(&self) -> Result<(), BackendError> {
        let opened = self.dial.open();
        let mut link = self.link.lock().unwrap_or_else(PoisonError::into_inner);
        match opened {
            Ok(jobs) => {
                *link = Link::Up(jobs);
                Ok(())
            }
            Err(e) => {
                *link = Link::Down(e.clone());
                Err(e)
            }
        }
    }

    pub(crate) fn run<T, F, Fut>(&self, make: F) -> Result<T, BackendError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, BackendError>> + 'static,
    {
        let jobs = match &*self.link.lock().unwrap_or_else(PoisonError::into_inner) {
            Link::Up(jobs) => jobs.clone(),
            Link::Down(e) => return Err(e.clone()),
        };
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |clients| {
            Box::pin(async move {
                let _ = reply.send(make(clients).await);
            })
        });
        jobs.send(job).map_err(|_| gone())?;
        answer.recv().map_err(|_| gone())?
    }
}

impl Dial {
    /// Start a worker thread that connects and then answers the jobs; wait until it is connected.
    fn open(&self) -> Result<tokio::sync::mpsc::UnboundedSender<Job>, BackendError> {
        let (jobs, inbox) = tokio::sync::mpsc::unbounded_channel();
        let (ready, started) = mpsc::channel();
        let how = match self {
            Self::Plain(socket) => Dial::Plain(socket.clone()),
            Self::Matching {
                socket,
                expected,
                starter,
            } => Dial::Matching {
                socket: socket.clone(),
                expected: expected.clone(),
                starter: Arc::clone(starter),
            },
        };
        thread::Builder::new()
            .name("files-rpc".into())
            .spawn(move || worker(&how, inbox, &ready))
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;
        started
            .recv()
            .map_err(|e| BackendError::Unavailable(e.to_string()))??;
        Ok(jobs)
    }

    async fn connect(&self) -> Result<Connection<files::Client>, BackendError> {
        match self {
            Self::Plain(socket) => connect::<files::Client>(socket).await.map_err(|e| {
                BackendError::Unavailable(format!(
                    "no files server on {}: {e}; start it with `fs-files serve`",
                    socket.display()
                ))
            }),
            Self::Matching {
                socket,
                expected,
                starter,
            } => connect_matching::<files::Client>(socket, expected, starter.as_ref())
                .await
                .map_err(|e| BackendError::Handshake(e.to_string())),
        }
    }
}

fn gone() -> BackendError {
    BackendError::Unavailable("the files server went away".into())
}

fn worker(
    dial: &Dial,
    mut inbox: tokio::sync::mpsc::UnboundedReceiver<Job>,
    ready: &mpsc::Sender<Result<(), BackendError>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(BackendError::Unavailable(e.to_string())));
            return;
        }
    };
    tokio::task::LocalSet::new().block_on(&runtime, async move {
        let connection = match dial.connect().await {
            Ok(c) => c,
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        let clients = match open(&connection.client).await {
            Ok(c) => Rc::new(c),
            Err(e) => {
                let _ = ready.send(Err(BackendError::Unavailable(e.to_string())));
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
