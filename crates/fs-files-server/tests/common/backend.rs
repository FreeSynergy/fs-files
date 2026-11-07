//! A backend served over a real socket, and the small calls tests make on it.

#![allow(dead_code)] // each test file uses a different subset

use std::sync::Arc;

use fs_files_core::BlobBackend;
use fs_files_server::BlobBackendServer;
use fs_interfaces::files_capnp::{blob_backend, spot};
use fs_interfaces::rpc::{bind_socket, connect, serve};

pub const CONFLICT: &str = fs_interfaces::files_capnp::NAME_CONFLICT_PROBLEM_TYPE;

pub async fn with_backend<F, Fut>(backend: Arc<dyn BlobBackend>, test: F)
where
    F: FnOnce(blob_backend::Client) -> Fut,
    Fut: Future<Output = ()>,
{
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("backend.sock");
            let root = BlobBackendServer::new(backend).into_client();
            let bound = bind_socket(&path).await.unwrap();
            let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
            let serving = tokio::task::spawn_local(serve(bound, root.client, async {
                let _ = stopped.await;
            }));
            let connection = connect::<blob_backend::Client>(&path).await.unwrap();
            test(connection.client.clone()).await;
            drop(connection);
            let _ = stop.send(());
            serving.await.unwrap().unwrap();
        })
        .await;
}

pub async fn put(
    b: &blob_backend::Client,
    key: &str,
    offset: u64,
    data: &[u8],
) -> capnp::Result<()> {
    let mut call = b.put_request();
    call.get().set_key(key);
    call.get().set_offset(offset);
    call.get().set_data(data);
    call.send().promise.await.map(|_| ())
}

pub async fn commit(b: &blob_backend::Client, key: &str) -> capnp::Result<(u64, String, i64)> {
    let mut call = b.commit_request();
    call.get().set_key(key);
    let answer = call.send().promise.await?;
    let info = answer.get()?.get_info()?;
    Ok((
        info.get_size(),
        info.get_checksum()?.to_string()?,
        info.get_modified(),
    ))
}

pub fn path_spot(mut s: spot::Builder<'_>, path: &str) {
    s.set_path(path);
}
