//! The table of problems, read back: every type of the server and the variant it must become.
//!
//! Pattern: none — a test file; it names what it tests: `problems.rs`, row by row, for the types the
//! real server can hardly be made to send (stored data, encryption); the others are covered over the
//! real socket.

use fs_files_core::Error;
use fs_interfaces::files_capnp::*;
use fs_interfaces::rpc::Problem;
use fs_interfaces::rpc::client::problems::PROBLEM_BASE as BASE;
use fs_objects::{Error as StoreError, ObjectId};

use crate::problems::error_of;

fn said(kind: &str, status: u16, detail: &str) -> Error {
    error_of(&Problem::new(kind, "t", detail, status).into_error())
}

#[test]
fn the_table_of_the_server_comes_back() {
    let id = ObjectId::new();
    let e = said(NAME_CONFLICT_PROBLEM_TYPE, 409, "name taken: a");
    assert!(
        matches!(&e, Error::Conflict(t) if t == "name taken: a"),
        "{e:?}"
    );
    assert_eq!(e.problem().unwrap().kind, NAME_CONFLICT_PROBLEM_TYPE);
    let e = said(INVALID_NAME_PROBLEM_TYPE, 422, "invalid name: empty");
    assert!(matches!(&e, Error::InvalidName(t) if t == "empty"), "{e:?}");
    assert_eq!(e.problem().unwrap().kind, INVALID_NAME_PROBLEM_TYPE);
    let e = said(
        CHANGED_OUTSIDE_PROBLEM_TYPE,
        409,
        "changed outside: size differs",
    );
    assert_eq!(
        e.problem().unwrap().kind,
        CHANGED_OUTSIDE_PROBLEM_TYPE,
        "{e:?}"
    );
    let e = said(CHANGED_OUTSIDE_PROBLEM_TYPE, 409, "invalid key: a symlink");
    assert_eq!(
        e.problem().unwrap().kind,
        CHANGED_OUTSIDE_PROBLEM_TYPE,
        "{e:?}"
    );
    let e = said(NOT_FOUND_PROBLEM_TYPE, 404, "not found: file x");
    assert!(matches!(&e, Error::NotFound(t) if t == "file x"), "{e:?}");
    let e = said(CONFLICT_PROBLEM_TYPE, 409, "folder is not empty");
    assert!(
        matches!(&e, Error::Conflict(t) if t == "folder is not empty"),
        "{e:?}"
    );
    assert!(e.problem().is_none(), "not a name clash");
    let e = said(
        WRONG_OFFSET_PROBLEM_TYPE,
        409,
        "wrong offset: the upload holds 12 bytes, the chunk starts at 5",
    );
    assert!(
        matches!(
            e,
            Error::Offset {
                expected: 12,
                got: 5
            }
        ),
        "{e:?}"
    );
    let e = said(WRONG_OFFSET_PROBLEM_TYPE, 409, "something else");
    assert!(matches!(e, Error::Conflict(_)), "{e:?}");
    let e = said(INVALID_PROBLEM_TYPE, 422, "configuration: no bucket");
    assert!(matches!(&e, Error::Config(t) if t == "no bucket"), "{e:?}");
    let e = said(
        ENCRYPTION_REQUIRED_PROBLEM_TYPE,
        409,
        &format!("file {id} needs a key"),
    );
    assert!(
        matches!(e, Error::Store(StoreError::EncryptionRequired(i)) if i == id),
        "{e:?}"
    );
    let e = said(STORED_DATA_PROBLEM_TYPE, 500, "json: bad");
    assert!(matches!(e, Error::Store(StoreError::Backend(_))), "{e:?}");
    let e = said(BACKEND_FAILED_PROBLEM_TYPE, 500, "backend: disk");
    assert!(matches!(&e, Error::Backend(t) if t == "disk"), "{e:?}");
    let e = said(&format!("{BASE}invalid-request"), 422, "invalid cursor");
    assert!(matches!(e, Error::Store(StoreError::Invalid(_))), "{e:?}");
    let e = said("whatever", 500, "boom");
    assert!(matches!(e, Error::Backend(_)), "{e:?}");
}

#[test]
fn unsupported_keeps_capability_and_reason() {
    let problem = Problem::new(
        "https://freesynergy.net/problems/unsupported",
        "t",
        "file.classify: not here",
        501,
    );
    let e = error_of(&problem.into_error());
    assert!(
        matches!(&e, Error::Unsupported(u) if u.capability() == "file.classify" && u.reason() == "not here"),
        "{e:?}"
    );
}
