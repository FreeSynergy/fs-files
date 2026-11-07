//! A folder as callers see it.
//!
//! Pattern: Value objects — [`Folder`] is the contract's `Folder`; its stored content is the parent id only.

use fs_store::{ObjectId, StoredObject};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A folder (`Folder` in `files.capnp`): an AS2 `Collection`.
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    /// The AS2 envelope as stored: what goes on the wire as `Folder.envelope`.
    pub envelope: fs_store::Envelope,
    /// Object id.
    pub id: ObjectId,
    /// `https://<instance>/files/<id>`.
    pub iri: String,
    /// Display name.
    pub name: String,
    /// The folder it is in; `None` = top level.
    pub parent: Option<ObjectId>,
}

/// The content of a stored folder object.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct FolderRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ObjectId>,
}

impl FolderRecord {
    /// Read the record of a stored folder.
    pub(crate) fn of(o: &StoredObject) -> Result<Self> {
        Ok(serde_json::from_value(o.content.clone())?)
    }
}

impl Folder {
    /// The caller's view of a stored folder.
    pub(crate) fn of(o: &StoredObject) -> Result<Self> {
        Ok(Self {
            envelope: o.envelope.clone(),
            id: o.envelope.id,
            iri: o
                .envelope
                .iri
                .clone()
                .ok_or_else(|| Error::Backend("folder without iri".into()))?,
            name: o.envelope.name.clone(),
            parent: FolderRecord::of(o)?.parent,
        })
    }
}
