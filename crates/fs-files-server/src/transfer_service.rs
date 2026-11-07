//! `Transfer` interface: start, resume and list uploads; open a file for reading.
//!
//! Pattern: Factory — each call hands out a capability (`Upload`, `Blob`) in front of the core's
//! object; the bytes themselves never pass through this file. Starting an upload writes (the session
//! is a record), so it runs to its end.

use std::rc::Rc;
use std::sync::Arc;

use capnp_rpc::new_client;
use fs_files_core::Transfer;
use fs_interfaces::files_capnp::transfer;
use fs_interfaces::rpc::read_object_id;

use crate::blob_service::BlobImpl;
use crate::errors::{finish, rpc_error};
use crate::upload_service::UploadImpl;
use crate::wire::report::{uploads_fitting, write_uploads};
use crate::wire::text::{self, MAX_SHORT, limited};
use crate::wire::{count, ids};

pub(crate) struct TransferImpl {
    pub(crate) transfer: Arc<dyn Transfer>,
}

type Done = Result<(), capnp::Error>;

impl transfer::Server for TransferImpl {
    async fn start_upload(
        self: Rc<Self>,
        params: transfer::StartUploadParams,
        mut results: transfer::StartUploadResults,
    ) -> Done {
        let p = params.get()?;
        let folder = ids::optional(p.get_folder()?)?;
        let name = text::name(p.get_name()?)?.to_string();
        let media_type = limited(p.get_media_type()?, "mediaType", MAX_SHORT)?.to_string();
        let on = ids::conflict(p.get_on_conflict())?;
        let transfer = self.transfer.clone();
        let (upload, id) = finish(async move {
            transfer
                .start_upload_as(folder, &name, &media_type, on)
                .await
        })
        .await?;
        let mut out = results.get();
        out.set_upload(new_client(UploadImpl::new(upload)));
        out.set_id(id.as_str());
        Ok(())
    }

    async fn resume_upload(
        self: Rc<Self>,
        params: transfer::ResumeUploadParams,
        mut results: transfer::ResumeUploadResults,
    ) -> Done {
        let id = limited(params.get()?.get_id()?, "id", MAX_SHORT)?;
        let upload = self.transfer.resume_upload(id).await.map_err(rpc_error)?;
        results
            .get()
            .set_upload(new_client(UploadImpl::new(upload)));
        Ok(())
    }

    async fn open(
        self: Rc<Self>,
        params: transfer::OpenParams,
        mut results: transfer::OpenResults,
    ) -> Done {
        let id = read_object_id(params.get()?.get_id()?)?;
        let blob = self.transfer.open(id).await.map_err(rpc_error)?;
        results.get().set_blob(new_client(BlobImpl { blob }));
        Ok(())
    }

    async fn pending_uploads(
        self: Rc<Self>,
        _: transfer::PendingUploadsParams,
        mut results: transfer::PendingUploadsResults,
    ) -> Done {
        let all = self.transfer.pending_uploads().await.map_err(rpc_error)?;
        let shown = uploads_fitting(&all);
        write_uploads(results.get().init_items(count(shown)?), &all, shown);
        Ok(())
    }
}
