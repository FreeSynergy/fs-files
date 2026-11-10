//! The `Backend` of the data type `StorageBackend` (ADR 0066): where the bytes of the files are kept.
//!
//! Pattern: Adapter — `files.capnp` `Backends` behind the `Backend` trait. The configuration text goes to the server as
//! it was typed; it can hold credentials, so it is never logged or put into an error here.

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_files_types::is_supported;

use crate::remote::{FilesRemote, problem, unsupported};
use crate::values::{storage_record, text};

/// The storage backends.
pub struct StorageBackendBackend(pub Arc<FilesRemote>);

impl Backend for StorageBackendBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        crate::listing::only_find_inputs(query, &[])?;
        self.0.run(|c| async move {
            let answer = c
                .backends
                .list_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let items = answer
                .get()
                .and_then(|r| r.get_items())
                .map_err(|e| problem(&e))?;
            items
                .iter()
                .map(|b| storage_record(b).map_err(|e| problem(&e)))
                .collect()
        })
    }

    fn run(&self, name: &str, input: &Values) -> Result<Outcome, BackendError> {
        if !is_supported("StorageBackend", name) {
            return Err(unsupported(name));
        }
        let (name, input) = (name.to_owned(), input.clone());
        self.0.run(move |c| async move {
            match name.as_str() {
                "create" => {
                    let mut call = c.backends.add_request();
                    call.get()
                        .set_config(text(&input, "config").unwrap_or_default());
                    let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                    let info = answer
                        .get()
                        .and_then(|r| r.get_info())
                        .map_err(|e| problem(&e))?;
                    Ok(Outcome {
                        id: Some(storage_record(info).map_err(|e| problem(&e))?.id),
                        values: Values::new(),
                    })
                }
                "delete" => {
                    let id = text(&input, "storage").ok_or(BackendError::NotFound)?;
                    let mut call = c.backends.remove_request();
                    call.get().set_id(id);
                    call.send().promise.await.map_err(|e| problem(&e))?;
                    Ok(Outcome {
                        id: Some(id.to_owned()),
                        values: Values::new(),
                    })
                }
                _ => Err(unsupported(&name)),
            }
        })
    }
}
