//! `Blob` capability: the bytes of one file, read in ranges.
//!
//! Pattern: Adapter — the capability other programs read through: the core's `Blob` trait as the wire
//! `Blob`; it shows no path and no backend. A read is clamped to what a message carries, which the contract
//! allows: fewer bytes than asked, an empty answer is the end. Nothing here reads a file whole.

use std::rc::Rc;
use std::sync::Arc;

use fs_files_core::Blob;
use fs_interfaces::common_capnp::blob;
use fs_interfaces::rpc::clamp_read;

use crate::errors::rpc_error;

pub(crate) struct BlobImpl {
    pub(crate) blob: Arc<dyn Blob>,
}

impl blob::Server for BlobImpl {
    async fn size(
        self: Rc<Self>,
        _: blob::SizeParams,
        mut results: blob::SizeResults,
    ) -> Result<(), capnp::Error> {
        let (bytes, media_type) = self.blob.size().await.map_err(rpc_error)?;
        let mut out = results.get();
        out.set_bytes(bytes);
        out.set_media_type(media_type.as_str());
        Ok(())
    }

    async fn read(
        self: Rc<Self>,
        params: blob::ReadParams,
        mut results: blob::ReadResults,
    ) -> Result<(), capnp::Error> {
        let p = params.get()?;
        let length = clamp_read(p.get_length());
        let data = self
            .blob
            .read(p.get_offset(), length)
            .await
            .map_err(rpc_error)?;
        results.get().set_data(&data);
        Ok(())
    }
}
