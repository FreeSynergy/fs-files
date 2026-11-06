//! A file as callers see it, and the record kept in the store.
//!
//! Pattern: Value objects + Adapter — [`File`] is the contract's `File`; `FileRecord` is the
//! JSON content of the stored `Document` (it also knows backend and key, which callers never see).

use fs_store::{ObjectId, StoredObject};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A file (`File` in `files.capnp`). It carries no path and no backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// Object id; other programs refer to the file by it.
    pub id: ObjectId,
    /// `https://<instance>/files/<id>`.
    pub iri: String,
    /// Display name (envelope `name`).
    pub name: String,
    /// Media type, for example `image/png`.
    pub media_type: String,
    /// Bytes.
    pub size: u64,
    /// `blake3:<hex>`.
    pub checksum: String,
    /// The folder it is in; `None` = top level.
    pub folder: Option<ObjectId>,
    /// In the trash.
    pub trashed: bool,
    /// DPV categories of sensitive data (ADR 0029).
    pub sensitive: Vec<String>,
}

/// The content of a stored file object.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileRecord {
    pub media_type: String,
    pub size: u64,
    pub checksum: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<ObjectId>,
    #[serde(default)]
    pub trashed: bool,
    /// Id of the backend that holds the bytes.
    pub backend: String,
    /// Key of the bytes in that backend.
    pub key: String,
}

impl FileRecord {
    /// Read the record of a stored file.
    pub(crate) fn of(o: &StoredObject) -> Result<Self> {
        Ok(serde_json::from_value(o.content.clone())?)
    }
}

impl File {
    /// The caller's view of a stored file.
    pub(crate) fn of(o: &StoredObject) -> Result<Self> {
        let r = FileRecord::of(o)?;
        Ok(Self {
            id: o.envelope.id,
            iri: o
                .envelope
                .iri
                .clone()
                .ok_or_else(|| Error::Backend("file without iri".into()))?,
            name: o.envelope.name.clone(),
            media_type: r.media_type,
            size: r.size,
            checksum: r.checksum,
            folder: r.folder,
            trashed: r.trashed,
            sensitive: o.envelope.sensitive.clone(),
        })
    }
}
