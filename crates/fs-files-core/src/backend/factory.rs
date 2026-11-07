//! Building backends from a configuration, and checking them before use.
//!
//! Pattern: Factory + Chain of checks — a [`BackendFactory`] per kind turns the JSON configuration
//! into a backend; `check_backend` then writes, reads and deletes a probe blob, so a wrong path or
//! wrong credentials are found before anything is swapped in (ADR 0045).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::{BlobBackend, LocalFolder};
use crate::error::{Error, Result};

/// Creates backends of one kind from their configuration.
#[async_trait]
pub trait BackendFactory: Send + Sync {
    /// The `kind` this factory builds (`local`, `s3`).
    fn kind(&self) -> &'static str;
    /// Build a backend from the configuration object (without `id`, `kind` and `active`).
    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>>;
    /// The folder `build` would use, checked, and nothing made. `None`: this kind has no folder of this
    /// machine, or does not say. `backend.add` refuses a folder that another backend has or lies in.
    async fn place(&self, _config: &Value) -> Result<Option<std::path::PathBuf>> {
        Ok(None)
    }
    /// Remove what `build` made for `place` (a folder that did not exist before) after a failed add.
    fn forget(&self, _place: &std::path::Path) {}
}

/// `{"kind": "local", "path": "/data/files"}`.
pub struct LocalFactory;

#[async_trait]
impl BackendFactory for LocalFactory {
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>> {
        let path = config
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Config("local backend needs \"path\"".into()))?;
        Ok(Arc::new(LocalFolder::open(path).await?))
    }
}

/// A parsed `backend.add` configuration.
pub struct BackendConfig {
    /// Id files will record.
    pub id: String,
    /// Kind to build.
    pub kind: String,
    /// Take new files from now on.
    pub active: bool,
    /// The whole object, for the factory.
    pub raw: Value,
}

impl BackendConfig {
    /// Parse and validate the common members. Error messages never repeat the text, which may hold credentials.
    pub fn parse(text: &str) -> Result<Self> {
        let raw: Value = serde_json::from_str(text)
            .map_err(|_| Error::Config("the configuration is not valid JSON".into()))?;
        let text_of = |k: &str| raw.get(k).and_then(Value::as_str).map(str::to_string);
        let id = text_of("id").ok_or_else(|| Error::Config("\"id\" is missing".into()))?;
        let kind = text_of("kind").ok_or_else(|| Error::Config("\"kind\" is missing".into()))?;
        let id_ok = !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !id_ok {
            return Err(Error::Config(
                "\"id\" must be 1-64 characters of a-z 0-9 - _".into(),
            ));
        }
        let active = raw.get("active").and_then(Value::as_bool).unwrap_or(false);
        Ok(Self {
            id,
            kind,
            active,
            raw,
        })
    }
}

/// Write, read and delete a small probe blob. Any failure means the backend is not usable.
pub async fn check_backend(backend: &dyn BlobBackend) -> Result<()> {
    let key = format!("fs-files-probe-{}", fs_store::ObjectId::new());
    let body = b"fs-files probe";
    let run = async {
        backend.put(&key, 0, body).await?;
        let info = backend.commit(&key).await?;
        let back = backend.get(&key, 0, 64).await?;
        if info.size != body.len() as u64 || back != body {
            return Err(Error::Backend("the probe blob came back different".into()));
        }
        Ok(())
    };
    let result = run.await;
    let cleanup = backend.delete(&key).await;
    result.and(cleanup)
}
