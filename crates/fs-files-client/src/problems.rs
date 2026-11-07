//! The problems of the server as the errors of the core.
//!
//! Pattern: Adapter — the inverse of `errors.rs` of `fs-files-server`: each RFC 9457 type becomes the
//! variant of `fs_files_core::Error` it came from, so the app's own reading of an error (`Error::problem`)
//! needs no change. The wire carries a type, a status and a text, not the variant's fields. The variant is
//! chosen by the TYPE, never by the words; the words are only given back their old shape where the
//! core's own `problem()` needs it (a name clash begins `name taken: `, a change from outside
//! `changed outside: `). The generic readers are those of `fs_interfaces::rpc::client::problems`.

use fs_files_core::Error;
use fs_interfaces::Unsupported;
use fs_interfaces::files_capnp::{
    BACKEND_FAILED_PROBLEM_TYPE, CHANGED_OUTSIDE_PROBLEM_TYPE, CONFLICT_PROBLEM_TYPE,
    ENCRYPTION_REQUIRED_PROBLEM_TYPE, INVALID_NAME_PROBLEM_TYPE, INVALID_PROBLEM_TYPE,
    NAME_CONFLICT_PROBLEM_TYPE, NOT_FOUND_PROBLEM_TYPE, STORED_DATA_PROBLEM_TYPE,
    WRONG_OFFSET_PROBLEM_TYPE,
};
use fs_interfaces::rpc::Problem;
use fs_interfaces::rpc::client::problems::{Failure, local_type, object_id_in, unsupported_of};
use fs_store::Error as StoreError;

/// The error of the core for what the server (or the wire) said.
pub(crate) fn error_of(e: &capnp::Error) -> Error {
    match Failure::of(e) {
        Failure::Problem(problem) => from_problem(&problem),
        Failure::Unimplemented => Unsupported::new(
            "files",
            "the server does not know this method of the contract",
        )
        .into(),
        Failure::Other(text) => Error::Backend(format!("files server: {text}")),
    }
}

/// `text` with `prefix` in front of it, unless it has it already.
fn with_prefix(prefix: &str, text: &str) -> String {
    if text.starts_with(prefix) {
        text.to_string()
    } else {
        format!("{prefix}{text}")
    }
}

/// `text` without a prefix that the error's own display adds again.
fn without<'a>(text: &'a str, prefixes: &[&str]) -> &'a str {
    prefixes
        .iter()
        .find_map(|p| text.strip_prefix(p))
        .unwrap_or(text)
}

fn from_problem(p: &Problem) -> Error {
    let detail = p.detail.as_str();
    if p.is_unsupported() {
        return unsupported_of(detail, "files").into();
    }
    if matches!(local_type(p), "invalid-request" | "too-large") {
        return Error::Store(StoreError::Invalid(detail.to_string()));
    }
    match p.problem_type.as_str() {
        NAME_CONFLICT_PROBLEM_TYPE => Error::Conflict(with_prefix("name taken: ", detail)),
        INVALID_NAME_PROBLEM_TYPE => {
            Error::InvalidName(without(detail, &["invalid name: ", "invalid key: "]).to_string())
        }
        CHANGED_OUTSIDE_PROBLEM_TYPE => Error::Backend(with_prefix("changed outside: ", detail)),
        NOT_FOUND_PROBLEM_TYPE => Error::NotFound(without(detail, &["not found: "]).to_string()),
        CONFLICT_PROBLEM_TYPE => Error::Conflict(detail.to_string()),
        WRONG_OFFSET_PROBLEM_TYPE => offset(detail),
        INVALID_PROBLEM_TYPE => Error::Config(without(detail, &["configuration: "]).to_string()),
        ENCRYPTION_REQUIRED_PROBLEM_TYPE => object_id_in(detail).map_or_else(
            || Error::Store(StoreError::Backend(format!("files server: {detail}"))),
            |id| Error::Store(StoreError::EncryptionRequired(id)),
        ),
        STORED_DATA_PROBLEM_TYPE => {
            Error::Store(StoreError::Backend(format!("files server: {detail}")))
        }
        BACKEND_FAILED_PROBLEM_TYPE => Error::Backend(without(detail, &["backend: "]).to_string()),
        _ => Error::Backend(format!("files server ({}): {detail}", p.status)),
    }
}

/// `the upload holds N bytes, the chunk starts at M`: the two numbers are the error's fields. A text
/// without them is a conflict with the server's words, not a guess at the numbers.
fn offset(detail: &str) -> Error {
    let mut numbers = detail
        .split(|c: char| !c.is_ascii_digit())
        .filter(|w| !w.is_empty())
        .filter_map(|w| w.parse::<u64>().ok());
    match (numbers.next(), numbers.next()) {
        (Some(expected), Some(got)) => Error::Offset { expected, got },
        _ => Error::Conflict(detail.to_string()),
    }
}
