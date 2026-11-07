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
use crate::backend::factory::{BackendFactory, Place};
use crate::backend::nested;
use crate::error::{Error, Result};
use crate::note::Note;

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
        let refused = |why: &str| Error::Config(Note::from(format!("the path of a local backend {why}")).because("path"));
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
    async fn resolve(&self, config: &Value) -> Result<(PathBuf, Place)> {
        let path = config
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("local backend needs \"path\"".into()))?;
        tokio::fs::create_dir_all(&self.base).await?;
        let base = tokio::fs::canonicalize(&self.base).await?;
        let at = Self::place(&base, path)?;
        let marker_new = at.is_dir() && !at.join(RESERVED_NAME).exists();
        // An existing folder that is no backend of its own may hold backends that are not mounted
        // (after a restart): it would take their files. Looked for, bounded; too big is refused.
        if marker_new && nested::holds_backend(&at) {
            return Err(Error::Config(
                "the folder of a local backend holds another backend, or is too big to check"
                    .into(),
            ));
        }
        let missing = |d: &&Path| !d.exists();
        let mut made: Vec<PathBuf> = at
            .ancestors()
            .take_while(missing)
            .map(Path::to_path_buf)
            .collect();
        made.reverse();
        Ok((
            base,
            Place {
                path: at,
                made,
                marker_new,
            },
        ))
    }
}

#[async_trait]
impl BackendFactory for ConfinedLocalFactory {
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn place(&self, config: &Value) -> Result<Option<Place>> {
        Ok(Some(self.resolve(config).await?.1))
    }

    fn forget(&self, place: &Place) {
        // A link where the folder was (swapped in since) is not ours to clean through.
        let is_link =
            std::fs::symlink_metadata(&place.path).is_ok_and(|m| m.file_type().is_symlink());
        if is_link {
            return;
        }
        if place.marker_new || !place.made.is_empty() {
            let _ = std::fs::remove_dir_all(place.path.join(RESERVED_NAME));
        }
        // Only folders this add made, innermost first, and only while they are empty.
        for dir in place.made.iter().rev() {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
        }
    }

    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>> {
        let (base, place) = self.resolve(config).await?;
        // `open` makes the folders it needs before the check below: a refusal found only there may
        // leave empty folders behind (a link swapped in between the check above and this open);
        // `forget` takes back what this add made. The base must be a folder only this program writes to.
        let backend = LocalFolder::open(&place.path).await?;
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
