//! `BlobBackend` interface: where bytes are. The provider side of the contract, for a backend that
//! runs as a program of its own and is used over the socket.
//!
//! Pattern: Adapter — wire request in, one call of the core's `BlobBackend`, wire response out. Nothing
//! more: the rules (never replace, paths of real names, pending and trash spots) are the backend's.
//! Writes run to their end. A read is clamped to what a message carries (fewer bytes than asked is
//! allowed, an empty answer is the end); a chunk to write above that is refused whole.

use std::{rc::Rc, sync::Arc};

use capnp_rpc::new_client;
use fs_files_core::{BlobBackend, Naming};
use fs_interfaces::files_capnp::blob_backend as b;
use fs_interfaces::rpc::{DEFAULT_PAGE_SIZE, Page, check_intake, clamp_read};

use crate::backend_wire::{key, read_keep, read_spot, scanned_weight, write_info, write_scanned};
use crate::errors::{finish, rpc_error};
use crate::wire::text::{MAX_SHORT, limited};
use crate::wire::{CONTRACT_VERSION, count};

/// A backend served over the socket.
pub struct BlobBackendServer {
    backend: Arc<dyn BlobBackend>,
}

impl BlobBackendServer {
    /// Serve this backend.
    pub fn new(backend: Arc<dyn BlobBackend>) -> Self {
        Self { backend }
    }

    /// The capability to hand to `fs_interfaces::rpc::serve`.
    #[must_use]
    pub fn into_client(self) -> b::Client {
        new_client(self)
    }
}

type Done = Result<(), capnp::Error>;

impl b::Server for BlobBackendServer {
    async fn support_report(
        self: Rc<Self>,
        _: b::SupportReportParams,
        mut r: b::SupportReportResults,
    ) -> Done {
        self.backend
            .capabilities()
            .write(CONTRACT_VERSION, r.get().init_report());
        Ok(())
    }

    async fn put(self: Rc<Self>, p: b::PutParams, _: b::PutResults) -> Done {
        let p = p.get()?;
        let (key, offset, data) = (
            key(p.get_key()?)?.to_string(),
            p.get_offset(),
            p.get_data()?,
        );
        check_intake(data.len())?;
        let (data, backend) = (data.to_vec(), self.backend.clone());
        finish(async move { backend.put(&key, offset, &data).await }).await
    }

    async fn commit(self: Rc<Self>, p: b::CommitParams, mut r: b::CommitResults) -> Done {
        let (key, backend) = (key(p.get()?.get_key()?)?.to_string(), self.backend.clone());
        let info = finish(async move { backend.commit(&key).await }).await?;
        write_info(r.get().init_info(), &info);
        Ok(())
    }

    async fn get(self: Rc<Self>, p: b::GetParams, mut r: b::GetResults) -> Done {
        let p = p.get()?;
        let length = clamp_read(p.get_length());
        let data = self
            .backend
            .get(key(p.get_key()?)?, p.get_offset(), length)
            .await;
        r.get().set_data(&data.map_err(rpc_error)?);
        Ok(())
    }

    async fn stat(self: Rc<Self>, p: b::StatParams, mut r: b::StatResults) -> Done {
        let found = self
            .backend
            .stat(key(p.get()?.get_key()?)?)
            .await
            .map_err(rpc_error)?;
        let mut out = r.get();
        out.set_found(found.is_some());
        if let Some(info) = found {
            write_info(out.init_info(), &info);
        }
        Ok(())
    }

    async fn delete(self: Rc<Self>, p: b::DeleteParams, _: b::DeleteResults) -> Done {
        let (key, backend) = (key(p.get()?.get_key()?)?.to_string(), self.backend.clone());
        finish(async move { backend.delete(&key).await }).await
    }

    async fn pending(self: Rc<Self>, p: b::PendingParams, mut r: b::PendingResults) -> Done {
        let held = self.backend.pending(key(p.get()?.get_key()?)?).await;
        r.get().set_bytes(held.map_err(rpc_error)?);
        Ok(())
    }

    async fn naming(self: Rc<Self>, _: b::NamingParams, mut r: b::NamingResults) -> Done {
        r.get().set_by_name(self.backend.naming() == Naming::ByName);
        Ok(())
    }

    async fn publish(self: Rc<Self>, p: b::PublishParams, mut r: b::PublishResults) -> Done {
        let p = p.get()?;
        let pending = limited(p.get_pending()?, "pending", MAX_SHORT)?.to_string();
        let (to, backend) = (key(p.get_key()?)?.to_string(), self.backend.clone());
        let info = finish(async move { backend.publish(&pending, &to).await }).await?;
        write_info(r.get().init_info(), &info);
        Ok(())
    }

    async fn relocate(self: Rc<Self>, p: b::RelocateParams, _: b::RelocateResults) -> Done {
        let p = p.get()?;
        let (from, to) = (read_spot(p.get_from()?)?, read_spot(p.get_to()?)?);
        let backend = self.backend.clone();
        finish(async move { backend.relocate(&from, &to).await }).await
    }

    async fn discard(self: Rc<Self>, p: b::DiscardParams, _: b::DiscardResults) -> Done {
        let (spot, backend) = (read_spot(p.get()?.get_spot()?)?, self.backend.clone());
        finish(async move { backend.discard(&spot).await }).await
    }

    async fn make_dir(self: Rc<Self>, p: b::MakeDirParams, _: b::MakeDirResults) -> Done {
        let p = p.get()?;
        let (path, parents) = (key(p.get_path()?)?.to_string(), p.get_parents());
        let backend = self.backend.clone();
        finish(async move { backend.make_dir(&path, parents).await }).await
    }

    async fn remove_dir(self: Rc<Self>, p: b::RemoveDirParams, _: b::RemoveDirResults) -> Done {
        let (path, backend) = (key(p.get()?.get_path()?)?.to_string(), self.backend.clone());
        finish(async move { backend.remove_dir(&path).await }).await
    }

    async fn scan(self: Rc<Self>, p: b::ScanParams, mut r: b::ScanResults) -> Done {
        let p = p.get()?;
        let all = self
            .backend
            .scan(key(p.get_dir()?)?)
            .await
            .map_err(rpc_error)?;
        let page = Page::of_weighted(
            all,
            p.get_cursor()?.get_opaque()?,
            DEFAULT_PAGE_SIZE,
            scanned_weight,
        )?;
        let mut out = r.get();
        write_scanned(
            out.reborrow().init_items(count(page.items.len())?),
            &page.items,
        );
        out.init_next().set_opaque(&page.next);
        Ok(())
    }

    async fn has(self: Rc<Self>, p: b::HasParams, mut r: b::HasResults) -> Done {
        let found = self.backend.has(&read_spot(p.get()?.get_spot()?)?).await;
        r.get().set_found(found.map_err(rpc_error)?);
        Ok(())
    }

    async fn sweep_pending(
        self: Rc<Self>,
        p: b::SweepPendingParams,
        mut r: b::SweepPendingResults,
    ) -> Done {
        let p = p.get()?;
        let keep = read_keep(p.get_keep()?)?;
        let (older, backend) = (p.get_older_than_secs(), self.backend.clone());
        let removed = finish(async move { backend.sweep_pending(&keep, older).await }).await?;
        r.get().set_removed(removed);
        Ok(())
    }

    async fn inspect(self: Rc<Self>, p: b::InspectParams, mut r: b::InspectResults) -> Done {
        let found = self
            .backend
            .inspect(&read_spot(p.get()?.get_spot()?)?)
            .await;
        let found = found.map_err(rpc_error)?;
        let mut out = r.get();
        out.set_found(found.is_some());
        if let Some(info) = found {
            write_info(out.init_info(), &info);
        }
        Ok(())
    }
}
