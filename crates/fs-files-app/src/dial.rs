//! The connection of `FilesRemote`: how it is made and the worker thread that owns it.
//!
//! Pattern: Strategy (`Dial`: plain or with the version handshake) + Proxy worker (one thread owns the `!Send`
//! Cap'n Proto connection and runs the jobs it is handed).

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::thread;

use fs_admin_frontend::BackendError;
use fs_interfaces::files_capnp::{backends, files, folders, search, transfer};
use fs_interfaces::rpc::client::call_limited;
use fs_interfaces::rpc::{Connection, Expected, ServerStarter, connect, connect_matching};

use crate::limits::{Limits, SERVER, timed_out};

/// The clients a job works with.
pub(crate) struct Clients {
    pub(crate) files: files::Client,
    pub(crate) folders: folders::Client,
    pub(crate) search: search::Client,
    pub(crate) transfer: transfer::Client,
    pub(crate) backends: backends::Client,
}

pub(crate) type Job = Box<dyn FnOnce(Rc<Clients>) -> Pin<Box<dyn Future<Output = ()>>> + Send>;

/// How the connection is made: plain (a server that is there, as the tests have it) or with the version handshake.
pub(crate) enum Dial {
    Plain(PathBuf),
    Matching {
        socket: PathBuf,
        expected: Expected,
        starter: Arc<dyn ServerStarter + Send + Sync>,
    },
}

impl Dial {
    /// Start a worker thread that connects and then answers the jobs; wait until it is connected.
    pub(crate) fn open(
        &self,
        limits: Limits,
    ) -> Result<tokio::sync::mpsc::UnboundedSender<Job>, BackendError> {
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
            .spawn(move || worker(&how, limits, inbox, &ready))
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

fn worker(
    dial: &Dial,
    limits: Limits,
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
        // The first questions have a limit too: a server that passed the handshake and then says nothing must not keep the
        // window from opening.
        let clients = match call_limited(SERVER, limits.start, open(&connection.client)).await {
            Ok(Ok(c)) => Rc::new(c),
            Ok(Err(e)) => {
                let _ = ready.send(Err(BackendError::Unavailable(e.to_string())));
                return;
            }
            Err(fault) => {
                let _ = ready.send(Err(timed_out(&fault)));
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
