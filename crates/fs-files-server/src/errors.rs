//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "name taken" from "not found" from "not possible" without reading a message. The three
//! problem types of `files.capnp` come from the core's own mapping (`Error::problem`); the others
//! are named here because the contract names none for them (reported, see `DESIGN.md`). What fails
//! on data already stored is not the caller's fault, so it is never a 422.

use std::future::Future;

use fs_files_core::Error;
use fs_interfaces::rpc::{Problem, failed, run_to_end, unsupported};
use fs_store::Error as Store;

pub(crate) const NOT_FOUND: &str = "https://freesynergy.net/problems/files/not-found";
pub(crate) const CONFLICT: &str = "https://freesynergy.net/problems/files/conflict";
pub(crate) const WRONG_OFFSET: &str = "https://freesynergy.net/problems/files/wrong-offset";
pub(crate) const INVALID: &str = "https://freesynergy.net/problems/files/invalid";
pub(crate) const ENCRYPTION_REQUIRED: &str =
    "https://freesynergy.net/problems/files/encryption-required";
pub(crate) const STORED_DATA: &str = "https://freesynergy.net/problems/files/stored-data-invalid";
pub(crate) const BACKEND_FAILED: &str = "https://freesynergy.net/problems/files/backend-failed";

fn problem(kind: &str, title: &str, detail: String, status: u16) -> capnp::Error {
    Problem::new(kind, title, detail, status).into_error()
}

/// A core error as an RPC error.
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    if let Some(named) = e.problem() {
        return problem(named.kind, named.title, named.detail, named.status);
    }
    let text = e.to_string();
    match e {
        Error::Unsupported(not_possible) => unsupported(&not_possible),
        Error::NotFound(_) | Error::Store(Store::NotFound(_)) => {
            problem(NOT_FOUND, "Not found", text, 404)
        }
        Error::Offset { .. } => problem(WRONG_OFFSET, "Wrong offset", text, 409),
        Error::Conflict(_) | Error::Store(Store::Conflict(_) | Store::OriginalImmutable(_)) => {
            problem(CONFLICT, "Conflict", text, 409)
        }
        Error::Config(_) | Error::Store(Store::Invalid(_)) => {
            problem(INVALID, "Invalid request", text, 422)
        }
        Error::Store(Store::EncryptionRequired(_)) => problem(
            ENCRYPTION_REQUIRED,
            "This store does not take sensitive files",
            text,
            409,
        ),
        Error::Json(_) | Error::Store(Store::Serde(_)) => {
            problem(STORED_DATA, "Stored data cannot be read", text, 500)
        }
        Error::Backend(_) | Error::Io(_) => {
            problem(BACKEND_FAILED, "The backend failed", text, 500)
        }
        // The name problems were answered above; these are the same, should that ever change.
        Error::InvalidName(_) | Error::InvalidKey(_) | Error::Store(_) => failed(text),
    }
}

/// Run a write of the provider to its end, also if the caller goes away meanwhile: dropped halfway
/// it would leave the disk and the database out of step.
pub(crate) async fn finish<T: 'static>(
    work: impl Future<Output = fs_files_core::Result<T>> + 'static,
) -> capnp::Result<T> {
    run_to_end(work).await?.map_err(rpc_error)
}
