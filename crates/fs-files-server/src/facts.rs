//! The facts behind a problem, as typed extensions.
//!
//! Pattern: none (a function that only computes from its argument). What a client used to cut out of
//! the detail text (`coding.md` §2a) is stated here as `id`, `kind`, `issue` and a few keys of the
//! files problem types; the detail text stays as it was. The table is in `DESIGN.md`.

use fs_files_core::Error;
use fs_interfaces::rpc::{Problem, problem_keys as keys};

/// The key of the name that is taken or not acceptable (`name-conflict`, `invalid-name`).
pub(crate) const NAME: &str = "name";
/// The key of the bytes the backend holds (`wrong-offset`): the offset the next chunk must have.
pub(crate) const EXPECTED: &str = "expected";
/// The key of the offset the caller sent (`wrong-offset`).
pub(crate) const GOT: &str = "got";

/// `problem` with every fact the core error `e` knows.
pub(crate) fn with_facts(mut problem: Problem, e: &Error) -> Problem {
    let facts = e.facts();
    if let Some(id) = &facts.id {
        problem = problem.with_ext(keys::ID, id);
    }
    if let Some(kind) = facts.kind {
        problem = problem.with_ext(keys::KIND, kind.noun());
    }
    if let Some(name) = &facts.name {
        problem = problem.with_ext(NAME, name);
    }
    if let Some((expected, got)) = facts.offsets {
        problem = problem
            .with_ext_u64(EXPECTED, expected)
            .with_ext_u64(GOT, got);
    }
    facts
        .issues
        .iter()
        .fold(problem, |p, issue| p.with_ext(keys::ISSUE, issue))
}
