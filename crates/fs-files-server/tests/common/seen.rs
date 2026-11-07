//! Files and entries as a consumer reads them from an answer.

#![allow(dead_code)] // each test file uses a different subset

use fs_interfaces::files_capnp::{entry, file};
use fs_interfaces::rpc::{read_envelope, read_uuid};
use fs_store::Envelope;
use uuid::Uuid;

/// A file as a consumer reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub id: Uuid,
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub checksum: String,
    pub folder: Option<Uuid>,
    pub trashed: bool,
    pub envelope: Envelope,
}

pub fn read_file(f: file::Reader<'_>) -> Seen {
    let envelope = read_envelope(f.get_envelope().unwrap()).unwrap();
    Seen {
        id: envelope.id.as_uuid(),
        name: envelope.name.clone(),
        media_type: f.get_media_type().unwrap().to_string().unwrap(),
        size: f.get_size(),
        checksum: f.get_checksum().unwrap().to_string().unwrap(),
        folder: read_uuid(f.get_folder().unwrap()),
        trashed: f.get_trashed(),
        envelope,
    }
}

/// An entry as (name, is a folder, id).
pub fn read_entry(e: entry::Reader<'_>) -> (String, bool, Uuid) {
    match e.which().unwrap() {
        entry::Which::File(f) => {
            let f = read_file(f.unwrap());
            (f.name, false, f.id)
        }
        entry::Which::Folder(f) => {
            let env = read_envelope(f.unwrap().get_envelope().unwrap()).unwrap();
            (env.name, true, env.id.as_uuid())
        }
    }
}
