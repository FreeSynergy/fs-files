//! A local backend that may only live below a base folder.
//!
//! Pattern: Factory + Specification — [`ConfinedLocalFactory`] builds `local` backends like
//! [`LocalFactory`](super::LocalFactory), but the configured `path` must name a place below the base:
//! relative, no `..`, no symlink on the way, not the base itself, not the program's own folder. A
//! program that takes `backend.add` from other programs uses it, so that a configuration cannot
//! point the program at any folder of the machine.

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
                Component::Normal(n) if n != RESERVED_NAME => parts.push(n),
                Component::Normal(_) => return Err(refused("names the program's own folder")),
                Component::CurDir => {}
                _ => return Err(refused("must be relative and stay below the base")),
            }
        }
        if parts.is_empty() {
            return Err(refused("must name a folder below the base"));
        }
        let mut at = base.to_path_buf();
        for part in parts {
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

#[async_trait]
impl BackendFactory for ConfinedLocalFactory {
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>> {
        let path = config
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("local backend needs \"path\"".into()))?;
        let base = tokio::fs::canonicalize(&self.base).await?;
        let at = Self::place(&base, path)?;
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
