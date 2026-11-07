//! Texts of a request, read with a limit.
//!
//! Pattern: Specification — what a text of a request may be, checked before the core sees it. A text
//! that is far beyond any legitimate one is refused at the door, so neither the core nor an error
//! message has to carry it.

use fs_interfaces::files_capnp::INVALID_NAME_PROBLEM_TYPE;
use fs_interfaces::rpc::{Problem, invalid_request};

/// A name above this is refused here as an invalid name; the exact rules (255 bytes and so on) are the core's.
const MAX_NAME_INTAKE: usize = 4096;
/// Media types, ids of uploads and backends.
pub(crate) const MAX_SHORT: usize = 1024;
/// Search terms and keys.
pub(crate) const MAX_TERM: usize = 8192;
/// A backend configuration.
pub(crate) const MAX_CONFIG: usize = 64 * 1024;
/// All sensitive categories of one file together.
pub(crate) const MAX_CATEGORIES: usize = 64 * 1024;

/// A name of a file or folder. Too long is the contract's `invalid-name`.
pub(crate) fn name(text: capnp::text::Reader<'_>) -> capnp::Result<&str> {
    let text = text.to_str()?;
    if text.len() > MAX_NAME_INTAKE {
        return Err(Problem::new(
            INVALID_NAME_PROBLEM_TYPE,
            "The name is not acceptable",
            format!("the name has {} bytes, at most 255 are allowed", text.len()),
            422,
        )
        .into_error());
    }
    Ok(text)
}

/// Any other text of a request, at most `max` bytes.
pub(crate) fn limited<'a>(
    text: capnp::text::Reader<'a>,
    what: &str,
    max: usize,
) -> capnp::Result<&'a str> {
    let text = text.to_str()?;
    if text.len() > max {
        return Err(invalid_request(format!(
            "{what} has {} bytes, at most {max} are allowed",
            text.len()
        )));
    }
    Ok(text)
}
