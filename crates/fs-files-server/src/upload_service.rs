//! `Upload` capability: one resumable upload.
//!
//! Pattern: Adapter + ordering Guard (RAII) — every call is one call of the core's `Upload`, which
//! owns the session state. Calls on one upload run in the order they arrived: a fair lock whose
//! guard is held by the work and dropped with it, not by the call. So a client may send chunks
//! without waiting for each answer; a chunk at the wrong offset is still refused by the core and
//! nothing is written. A chunk above what a message can safely carry in is refused whole.

use std::rc::Rc;
use std::sync::Arc;

use fs_files_core::Upload;
use fs_interfaces::files_capnp::upload;
use fs_interfaces::rpc::check_intake;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::errors::{finish, rpc_error};
use crate::wire::entry::write_file;

pub(crate) struct UploadImpl {
    upload: Arc<dyn Upload>,
    order: Arc<Mutex<()>>,
}

impl UploadImpl {
    pub(crate) fn new(upload: Arc<dyn Upload>) -> Self {
        Self {
            upload,
            order: Arc::default(),
        }
    }

    async fn turn(&self) -> OwnedMutexGuard<()> {
        self.order.clone().lock_owned().await
    }
}

type Done = Result<(), capnp::Error>;

impl upload::Server for UploadImpl {
    async fn write(self: Rc<Self>, params: upload::WriteParams, _: upload::WriteResults) -> Done {
        let p = params.get()?;
        let (offset, data) = (p.get_offset(), p.get_data()?);
        check_intake(data.len())?;
        let data = data.to_vec();
        let (turn, upload) = (self.turn().await, self.upload.clone());
        finish(async move {
            let _turn = turn;
            upload.write(offset, &data).await
        })
        .await
    }

    async fn received(
        self: Rc<Self>,
        _: upload::ReceivedParams,
        mut results: upload::ReceivedResults,
    ) -> Done {
        let _turn = self.turn().await;
        let bytes = self.upload.received().await.map_err(rpc_error)?;
        results.get().set_bytes(bytes);
        Ok(())
    }

    async fn finish(
        self: Rc<Self>,
        _: upload::FinishParams,
        mut results: upload::FinishResults,
    ) -> Done {
        let (turn, upload) = (self.turn().await, self.upload.clone());
        let file = finish(async move {
            let _turn = turn;
            upload.finish().await
        })
        .await?;
        write_file(results.get().init_file(), &file);
        Ok(())
    }

    async fn abort(self: Rc<Self>, _: upload::AbortParams, _: upload::AbortResults) -> Done {
        let (turn, upload) = (self.turn().await, self.upload.clone());
        finish(async move {
            let _turn = turn;
            upload.abort().await
        })
        .await
    }
}
