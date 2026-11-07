//! The configuration of an S3 backend.
//!
//! Pattern: Value object — parsed from the `backend.add` JSON, checked, and turned into
//! a client. The credentials live only here and in the client; `Debug` hides them and no error
//! message repeats the input.

use std::sync::Arc;

use std::time::Duration;

use object_store::RetryConfig;
use object_store::aws::AmazonS3Builder;
use serde::Deserialize;

use super::adapter::ObjectStoreApi;
use super::backend::{MIN_PART_SIZE, S3Backend, S3Options};
use crate::error::{Error, Result};

/// `{"kind": "s3", "bucket": .., "endpoint": .., "access_key": .., "secret_key": ..}`.
#[derive(Clone, Deserialize)]
pub struct S3Config {
    /// Bucket name.
    pub bucket: String,
    /// Endpoint URL of a non-AWS store, for example `http://127.0.0.1:9000`. Empty = AWS.
    #[serde(default)]
    pub endpoint: String,
    /// Region (default `us-east-1`; most other stores ignore it).
    #[serde(default = "default_region")]
    pub region: String,
    /// Access key id.
    pub access_key: String,
    /// Secret access key.
    pub secret_key: String,
    /// Session token, if the credentials are temporary.
    #[serde(default)]
    pub session_token: Option<String>,
    /// Key prefix inside the bucket.
    #[serde(default)]
    pub prefix: String,
    /// Part size in MiB (default 8, at least 5).
    #[serde(default = "default_part_mib")]
    pub part_size_mib: u64,
    /// Path-style addressing `endpoint/bucket/key` (default true; needed by most self-hosted stores).
    #[serde(default = "yes")]
    pub path_style: bool,
}

fn default_region() -> String {
    "us-east-1".into()
}
fn default_part_mib() -> u64 {
    8
}
fn yes() -> bool {
    true
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "<redacted>"),
            )
            .finish_non_exhaustive()
    }
}

impl S3Config {
    /// Read the configuration object. The error never contains the input.
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        let cfg: Self = serde_json::from_value(value.clone()).map_err(|_| {
            Error::Config("s3 needs \"bucket\", \"access_key\" and \"secret_key\" (text); check the field types".into())
        })?;
        if cfg.bucket.is_empty() || cfg.access_key.is_empty() || cfg.secret_key.is_empty() {
            return Err(Error::Config(
                "s3: bucket, access_key and secret_key must not be empty".into(),
            ));
        }
        if cfg.part_size_mib.saturating_mul(1 << 20) < MIN_PART_SIZE {
            return Err(Error::Config("s3: part_size_mib must be at least 5".into()));
        }
        Ok(cfg)
    }

    /// Connect: build the client and the backend. Nothing is sent yet; `check_backend` does the first call.
    pub fn connect(&self) -> Result<S3Backend> {
        let mut b = AmazonS3Builder::new()
            .with_bucket_name(&self.bucket)
            .with_region(&self.region)
            .with_access_key_id(&self.access_key)
            .with_secret_access_key(&self.secret_key)
            .with_virtual_hosted_style_request(!self.path_style);
        if let Some(t) = &self.session_token {
            b = b.with_token(t);
        }
        if !self.endpoint.is_empty() {
            b = b
                .with_endpoint(&self.endpoint)
                .with_allow_http(self.endpoint.starts_with("http://"));
        }
        let retry = RetryConfig {
            max_retries: 3,
            retry_timeout: Duration::from_secs(30),
            ..RetryConfig::default()
        };
        let store = b.with_retry(retry).build().map_err(|_| {
            Error::Config("s3: the client could not be built (check endpoint and region)".into())
        })?;
        let options = S3Options {
            prefix: self.prefix.clone(),
            part_size: self.part_size_mib << 20,
        };
        S3Backend::new(Arc::new(ObjectStoreApi(store)), &options)
    }
}
