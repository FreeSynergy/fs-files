//! `FilesRemote`: the files server behind a socket.
//!
//! Pattern: Proxy — a Cap'n Proto client lives on one thread, so a worker thread owns the connection and every call is
//! a job handed to it; the caller waits on a plain channel (the window thread is not inside a runtime). The `Backend`
//! of each data type for the generated frontend is implemented on it in the `backend_*` modules.

use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

use fs_admin_frontend::BackendError;
use fs_interfaces::rpc::client::{Fault, call_limited};
use fs_interfaces::rpc::{Expected, Problem, ServerStarter};
use fs_types::{RecordIssue, RecordProblem, TypeProblem};

pub(crate) use crate::dial::Clients;
use crate::dial::{Dial, Job};
pub use crate::limits::Limits;
use crate::limits::{GRACE, LONG_COMMANDS, SERVER, timed_out};

/// The state of the connection: a worker that answers, or why there is none (the window shows it, "Try again").
enum Link {
    Up(tokio::sync::mpsc::UnboundedSender<Job>),
    Down(BackendError),
}

/// The files server behind a socket.
pub struct FilesRemote {
    dial: Dial,
    limits: Limits,
    link: Mutex<Link>,
}

impl FilesRemote {
    /// Connect to the server that is on `socket`, with no version check and no start (what the tests need).
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect(socket: &Path) -> Result<Self, String> {
        Self::connect_with(socket, Limits::default())
    }

    /// [`FilesRemote::connect`] with other [`Limits`].
    ///
    /// # Errors
    /// The text for the person when nothing answers.
    pub fn connect_with(socket: &Path, limits: Limits) -> Result<Self, String> {
        let dial = Dial::Plain(socket.to_path_buf());
        match dial.open(limits) {
            Ok(jobs) => Ok(Self {
                dial,
                limits,
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
        Self::matching_with(socket, starter, Limits::default())
    }

    /// [`FilesRemote::matching`] with other [`Limits`]. A server that passes the handshake and then does not answer
    /// the first questions is reported like a failed handshake after [`Limits::start`], not waited for.
    pub fn matching_with(
        socket: &Path,
        starter: Arc<dyn ServerStarter + Send + Sync>,
        limits: Limits,
    ) -> Self {
        let dial = Dial::Matching {
            socket: socket.to_path_buf(),
            expected: Expected::new(env!("CARGO_PKG_VERSION")),
            starter,
        };
        let link = match dial.open(limits) {
            Ok(jobs) => Link::Up(jobs),
            Err(e) => Link::Down(e),
        };
        Self {
            dial,
            limits,
            link: Mutex::new(link),
        }
    }

    /// Connect again (after a failed handshake, or a server that went away) and keep the new connection.
    ///
    /// # Errors
    /// [`BackendError::Handshake`] (again) or [`BackendError::Unavailable`].
    pub fn reconnect(&self) -> Result<(), BackendError> {
        let opened = self.dial.open(self.limits);
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

    /// A call with the default limit ([`Limits::call`]).
    pub(crate) fn run<T, F, Fut>(&self, make: F) -> Result<T, BackendError>
    where
        T: Send + 'static,
        F: FnOnce(Rc<Clients>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, BackendError>> + 'static,
    {
        self.run_within(self.limits.call, make)
    }

    /// The limit of the command `name`: [`Limits::long`] for a transfer or a walk, else [`Limits::call`].
    pub(crate) fn limit_of(&self, name: &str) -> Duration {
        if LONG_COMMANDS.contains(&name) {
            self.limits.long
        } else {
            self.limits.call
        }
    }

    /// The limit of the questions of the start ([`Limits::start`]).
    pub(crate) fn start_limit(&self) -> Duration {
        self.limits.start
    }

    /// A call that is given up after `limit`: the server did not answer, the caller gets a visible error and the
    /// connection stays usable (the late answer is dropped). A write may still have happened on the server.
    pub(crate) fn run_within<T, F, Fut>(&self, limit: Duration, make: F) -> Result<T, BackendError>
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
                let done = call_limited(SERVER, limit, make(clients))
                    .await
                    .unwrap_or_else(|fault| Err(timed_out(&fault)));
                let _ = reply.send(done);
            })
        });
        jobs.send(job).map_err(|_| gone())?;
        // The job ends itself at `limit`; this is the belt for a worker that does not run it at all.
        match answer.recv_timeout(limit + GRACE) {
            Ok(done) => done,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(timed_out(&Fault::TimedOut {
                server: SERVER,
                limit,
            })),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(gone()),
        }
    }
}

fn gone() -> BackendError {
    BackendError::Unavailable("the files server went away".into())
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
