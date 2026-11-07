//! Errors of the core as RPC errors.
//!
//! Pattern: Adapter — each error the core can name becomes a typed RFC 9457 problem, so a consumer
//! can tell "name taken" from "not found" from "not possible" without reading a message. The three
//! problem types of `files.capnp` come from the core's own mapping (`Error::problem`) and from the
//! constants of the contract (since `fs-interfaces` v0.35.0). What fails
//! on data already stored is not the caller's fault, so it is never a 422.

use std::future::Future;

use fs_files_core::Error;
use fs_interfaces::rpc::{Problem, failed, problem_keys, run_to_end, unsupported};
use fs_objects::Error as Store;

use crate::facts::with_facts;

use fs_interfaces::files_capnp::{
    BACKEND_FAILED_PROBLEM_TYPE as BACKEND_FAILED, CONFLICT_PROBLEM_TYPE as CONFLICT,
    ENCRYPTION_REQUIRED_PROBLEM_TYPE as ENCRYPTION_REQUIRED, INVALID_PROBLEM_TYPE as INVALID,
    NOT_FOUND_PROBLEM_TYPE as NOT_FOUND, STORED_DATA_PROBLEM_TYPE as STORED_DATA,
    WRONG_OFFSET_PROBLEM_TYPE as WRONG_OFFSET,
};

/// A core error as an RPC error.
#[allow(clippy::needless_pass_by_value)] // used as `map_err(rpc_error)`
pub(crate) fn rpc_error(e: Error) -> capnp::Error {
    let make = |kind: &str, title: &str, detail: String, status: u16| {
        with_facts(Problem::new(kind, title, detail, status), &e).into_error()
    };
    if let Some(named) = e.problem() {
        return make(named.kind, named.title, named.detail, named.status);
    }
    let text = e.to_string();
    match &e {
        Error::Unsupported(not_possible) => unsupported(not_possible),
        Error::NotFound(_) | Error::Store(Store::NotFound(_)) => {
            make(NOT_FOUND, "Not found", text, 404)
        }
        Error::Offset { .. } => make(WRONG_OFFSET, "Wrong offset", text, 409),
        Error::Conflict(_) | Error::Store(Store::Conflict(_) | Store::OriginalImmutable(_)) => {
            make(CONFLICT, "Conflict", text, 409)
        }
        Error::Config(_) | Error::Store(Store::Invalid(_)) => {
            make(INVALID, "Invalid request", text, 422)
        }
        Error::Store(Store::EncryptionRequired(_)) => make(
            ENCRYPTION_REQUIRED,
            "This store does not take sensitive files",
            text,
            409,
        ),
        Error::Json(_) | Error::Store(Store::Serde(_)) => {
            make(STORED_DATA, "Stored data cannot be read", text, 500)
        }
        Error::Backend(_) | Error::Io(_) => {
            let named_already = e.facts().kind.is_some();
            let failed = Problem::new(BACKEND_FAILED, "The backend failed", text, 500);
            let failed = with_facts(failed, &e);
            // A failure that names no object is the backend's own.
            let failed = if named_already {
                failed
            } else {
                failed.with_ext(problem_keys::KIND, "backend")
            };
            failed.into_error()
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

#[cfg(test)]
mod tests {
    use fs_interfaces::UNSUPPORTED_PROBLEM_TYPE;
    use fs_interfaces::Unsupported;
    use fs_interfaces::files_capnp::{
        CHANGED_OUTSIDE_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, NAME_CONFLICT_PROBLEM_TYPE,
    };
    use fs_objects::ObjectId;

    use super::*;

    fn seen(e: Error) -> (String, u16) {
        let p = Problem::from_error(&rpc_error(e)).expect("a typed problem");
        (p.problem_type, p.status)
    }

    fn is(kind: &str, status: u16) -> (String, u16) {
        (kind.to_string(), status)
    }

    #[test]
    fn every_error_of_the_core_keeps_its_type() {
        let id = ObjectId::new();
        let serde = serde_json::from_str::<u8>("x").unwrap_err();
        let io = std::io::Error::other("disk");
        assert_eq!(
            seen(Error::Conflict("name taken: x".into())),
            is(NAME_CONFLICT_PROBLEM_TYPE, 409)
        );
        assert_eq!(
            seen(Error::InvalidName("x".into())),
            is(INVALID_NAME_PROBLEM_TYPE, 422)
        );
        assert_eq!(
            seen(Error::InvalidKey("x".into())),
            is(INVALID_NAME_PROBLEM_TYPE, 422)
        );
        let outside = Error::NotFound("changed outside: x".into());
        assert_eq!(seen(outside), is(CHANGED_OUTSIDE_PROBLEM_TYPE, 409));
        assert_eq!(seen(Error::NotFound("x".into())), is(NOT_FOUND, 404));
        assert_eq!(seen(Store::NotFound(id).into()), is(NOT_FOUND, 404));
        assert_eq!(seen(Error::Conflict("x".into())), is(CONFLICT, 409));
        assert_eq!(seen(Store::Conflict("x".into()).into()), is(CONFLICT, 409));
        assert_eq!(seen(Store::OriginalImmutable(id).into()), is(CONFLICT, 409));
        assert_eq!(
            seen(Error::Offset {
                expected: 1,
                got: 2
            }),
            is(WRONG_OFFSET, 409)
        );
        assert_eq!(seen(Error::Config("x".into())), is(INVALID, 422));
        assert_eq!(seen(Store::Invalid("x".into()).into()), is(INVALID, 422));
        assert_eq!(
            seen(Store::EncryptionRequired(id).into()),
            is(ENCRYPTION_REQUIRED, 409)
        );
        assert_eq!(seen(Error::Json(serde)), is(STORED_DATA, 500));
        assert_eq!(seen(Error::Backend("x".into())), is(BACKEND_FAILED, 500));
        assert_eq!(seen(Error::Io(io)), is(BACKEND_FAILED, 500));
        let no = Unsupported::new("file.versions", "not built");
        assert_eq!(seen(no.into()), is(UNSUPPORTED_PROBLEM_TYPE, 501));
    }
}
