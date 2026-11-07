//! A local backend that may only live below a base folder.
//!
//! Pattern: Factory + Specification — [`ConfinedLocalFactory`] builds `local` backends like
//! [`LocalFactory`](super::LocalFactory), but the configured `path` must name a place below the base:
//! relative, no `..`, no symlink on the way, not the base itself, not the program's own folder. A
//! program that takes `backend.add` from other programs uses it, so that a configuration cannot
//! point the program at any folder of the machine. The base must lie outside the tree of the main
//! backend (a rescan of that would take the files of the others), and nobody else may write below
//! it: the checks are by path, not by `openat2`.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::{BlobBackend, LocalFolder, RESERVED_NAME};
use crate::backend::factory::BackendFactory;
use crate::error::{Error, Result};

/// `{"kind": "local", "path": "second"}`: the path is relative to the base.
pub struct ConfinedLocalFactory {
    base: PathBuf,
}

impl ConfinedLocalFactory {
    /// Local backends below `base`.
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    /// The place `path` names below the base, if it is one. Nothing is created here.
    fn place(base: &Path, path: &str) -> Result<PathBuf> {
        let refused = |why: &str| Error::Config(format!("the path of a local backend {why}"));
        let relative = Path::new(path);
        let mut parts = Vec::new();
        for c in relative.components() {
            match c {
                Component::Normal(n) if n.to_string_lossy().eq_ignore_ascii_case(RESERVED_NAME) => {
                    return Err(refused("names the program's own folder"));
                }
                Component::Normal(n) => parts.push(n),
                Component::CurDir => {}
                _ => return Err(refused("must be relative and stay below the base")),
            }
        }
        if parts.is_empty() {
            return Err(refused("must name a folder below the base"));
        }
        let mut at = base.to_path_buf();
        for part in parts {
            // A folder that is a backend's own root (it has the program's folder) holds that backend:
            // another one inside it would be part of it.
            if at != base && at.join(RESERVED_NAME).exists() {
                return Err(refused("lies inside another backend"));
            }
            at.push(part);
            match std::fs::symlink_metadata(&at) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(refused("passes a symbolic link"));
                }
                Ok(m) if !m.is_dir() => return Err(refused("is not a folder")),
                _ => {}
            }
        }
        Ok(at)
    }
}

impl ConfinedLocalFactory {
    /// The canonical base (made if it is not there: it is the program's own folder) and the place.
    async fn resolve(&self, config: &Value) -> Result<(PathBuf, PathBuf)> {
        let path = config
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("local backend needs \"path\"".into()))?;
        tokio::fs::create_dir_all(&self.base).await?;
        let base = tokio::fs::canonicalize(&self.base).await?;
        let at = Self::place(&base, path)?;
        Ok((base, at))
    }
}

#[async_trait]
impl BackendFactory for ConfinedLocalFactory {
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn place(&self, config: &Value) -> Result<Option<PathBuf>> {
        Ok(Some(self.resolve(config).await?.1))
    }

    fn forget(&self, place: &Path) {
        let _ = std::fs::remove_dir_all(place);
        // The parents made on the way, as far as they are empty; the base itself stays.
        let mut parent = place.parent();
        while let Some(dir) = parent.filter(|d| d.starts_with(&self.base) && *d != self.base) {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
            parent = dir.parent();
        }
    }

    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>> {
        let (base, at) = self.resolve(config).await?;
        // `open` makes the folders it needs before the check below: a refusal found only there may
        // leave empty folders behind (a link swapped in between the check above and this open);
        // it never mounts the backend. The base must be a folder only this program writes to.
        let backend = LocalFolder::open(&at).await?;
        // Belt and braces: a link made between the check and the open must not carry it out.
        if !backend.root().starts_with(&base) || backend.root() == base {
            return Err(Error::Config(
                "the path of a local backend must stay below the base".into(),
            ));
        }
        Ok(Arc::new(backend))
    }
}

/// The mounted backend whose folder `place` is, holds, or lies in. A place inside the program's own
/// folder of a backend (`.fs-files`) is no part of that backend.
pub(crate) fn overlapping<'a>(place: &Path, mounted: &'a [PathBuf]) -> Option<&'a PathBuf> {
    mounted.iter().find(|m| {
        let below = place.strip_prefix(m).ok();
        let inside = below.is_some_and(|rest| !rest.starts_with(RESERVED_NAME));
        inside || m.starts_with(place)
    })
}
