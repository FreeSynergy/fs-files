//! File, folder and session <-> stored object: envelope derivation and index fields.
//!
//! Pattern: Adapter — maps our records onto the AS2 envelope + index fields that `fs-store`
//! understands (ADR 0020); the envelope `name` is derived, never typed twice (ADR 0032).

use fs_store::{Envelope, ObjectId, StoredObject};

use super::consts::{
    CONTEXT, FILE_CONTENT, FOLDER_CONTENT, FOLDER_TYPE, ROOT, UPLOAD_CONTENT, UPLOAD_TYPE,
};
use super::handle::FileRepo;
use super::session::SessionRecord;
use crate::model::{FileRecord, FolderRecord};

/// AS2 type of a file by its media type: `Image`, `Video`, `Audio`, else `Document`.
pub(super) fn as2_type(media_type: &str) -> &'static str {
    match media_type.split('/').next() {
        Some("image") => "Image",
        Some("video") => "Video",
        Some("audio") => "Audio",
        _ => "Document",
    }
}

/// Index value for a folder.
pub(super) fn folder_key(folder: Option<ObjectId>) -> String {
    folder.map_or_else(|| ROOT.to_string(), |f| f.to_string())
}

impl FileRepo {
    fn envelope(&self, ty: &str, name: &str) -> Envelope {
        let mut e = Envelope::new(ty, name);
        e.iri = Some(self.iri(e.id));
        e.context = Some(CONTEXT.into());
        e.attributed_to = Some(self.actor.clone());
        e.to = vec![self.actor.clone()];
        e
    }

    pub(super) fn file_object(&self, name: &str, r: &FileRecord) -> StoredObject {
        let mut o = StoredObject::new(
            self.envelope(as2_type(&r.media_type), name),
            FILE_CONTENT,
            serde_json::to_value(r).unwrap_or_default(),
        );
        index_file(&mut o, r);
        o
    }

    pub(super) fn folder_object(&self, name: &str, parent: Option<ObjectId>) -> StoredObject {
        let mut o = StoredObject::new(
            self.envelope(FOLDER_TYPE, name),
            FOLDER_CONTENT,
            serde_json::to_value(FolderRecord { parent }).unwrap_or_default(),
        );
        index_folder(&mut o, parent);
        o
    }

    pub(super) fn session_object(&self, name: &str, r: &SessionRecord) -> StoredObject {
        let mut o = StoredObject::new(
            self.envelope(UPLOAD_TYPE, name),
            UPLOAD_CONTENT,
            serde_json::to_value(r).unwrap_or_default(),
        );
        o.index.insert("kind".into(), vec!["upload".into()]);
        o.index.insert("backend".into(), vec![r.backend.clone()]);
        o
    }
}

/// Rewrite the content and index of a file object after `r` changed.
pub(super) fn set_file(o: &mut StoredObject, r: &FileRecord) {
    o.content = serde_json::to_value(r).unwrap_or_default();
    index_file(o, r);
    o.envelope.updated = chrono::Utc::now();
}

/// Rewrite the content and index of a folder object.
pub(super) fn set_folder(o: &mut StoredObject, parent: Option<ObjectId>) {
    o.content = serde_json::to_value(FolderRecord { parent }).unwrap_or_default();
    index_folder(o, parent);
    o.envelope.updated = chrono::Utc::now();
}

fn index_file(o: &mut StoredObject, r: &FileRecord) {
    o.envelope.object_type = as2_type(&r.media_type).into();
    let one = |v: String| vec![v];
    o.index.insert("kind".into(), one("file".into()));
    o.index
        .insert("name_exact".into(), one(o.envelope.name.clone()));
    o.index
        .insert("media_type".into(), one(r.media_type.clone()));
    o.index.insert("folder".into(), one(folder_key(r.folder)));
    o.index.insert("trashed".into(), one(r.trashed.to_string()));
    o.index.insert("backend".into(), one(r.backend.clone()));
}

fn index_folder(o: &mut StoredObject, parent: Option<ObjectId>) {
    let one = |v: String| vec![v];
    o.index.insert("kind".into(), one("folder".into()));
    o.index
        .insert("name_exact".into(), one(o.envelope.name.clone()));
    o.index.insert("folder".into(), one(folder_key(parent)));
    o.index.insert("trashed".into(), one("false".into()));
}
