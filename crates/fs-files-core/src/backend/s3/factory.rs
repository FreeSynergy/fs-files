//! The factory for `{"kind": "s3"}` configurations.
//!
//! Pattern: Factory — plugs into `backend.add` next to the local one.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::config::S3Config;
use crate::backend::{BackendFactory, BlobBackend};
use crate::error::Result;

/// Builds [`S3Backend`](super::S3Backend)s from their JSON configuration.
pub struct S3Factory;

#[async_trait]
impl BackendFactory for S3Factory {
    fn kind(&self) -> &'static str {
        "s3"
    }

    async fn build(&self, config: &Value) -> Result<Arc<dyn BlobBackend>> {
        Ok(Arc::new(S3Config::from_json(config)?.connect()?))
    }
}
