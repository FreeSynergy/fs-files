//! Helpers for tests that look at the real directory behind the local backend.
#![allow(clippy::needless_pass_by_value)] // tests: helper signatures kept simple, callers pass owned values

use std::path::{Path, PathBuf};

use super::Env;
use fs_files_core::Error;

pub fn root(e: &Env) -> PathBuf {
    e.dir.path().join("blobs").canonicalize().unwrap()
}

pub fn on_disk(e: &Env, rel: &str) -> Vec<u8> {
    std::fs::read(root(e).join(rel)).unwrap()
}

pub fn exists(e: &Env, rel: &str) -> bool {
    root(e).join(rel).symlink_metadata().is_ok()
}

/// Everything below `dir` except the program's own folder, as relative paths.
pub fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, at: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            // Always written with `/`, so a test reads the same on every system.
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == ".fs-files" {
                continue;
            }
            out.push(rel);
            if p.is_dir() && !p.is_symlink() {
                walk(base, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

pub fn is_conflict<T>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Conflict(_)))
}
