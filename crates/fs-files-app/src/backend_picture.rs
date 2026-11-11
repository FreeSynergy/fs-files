//! The `Backend` of the data type `Picture` and of `Album` (the gallery facet, ADR 0066).
//!
//! Pattern: Adapter — a picture is a file whose media type is `image/…`: the list is `find` with that prefix, and the
//! two commands the core has (`download`, `sensitiveMark`) are the file's. The core has no metadata object for a
//! picture and none for an album; everything else answers `unsupported:<name>` and the control is locked. An album
//! list is empty because there is no album object (not because nothing could be read).

use std::sync::Arc;

use fs_admin_frontend::{Backend, BackendError, Outcome, Record, Values};
use fs_files_types::is_supported;
use fs_types::Edit;

use crate::backend_file::FileBackend;
use crate::listing::{FIND_KEYS, find_all, only_find_inputs};
use crate::remote::{FilesRemote, unsupported};
use crate::transfer::download;
use crate::values::{Found, need_id, text, with_record};

/// The pictures.
pub struct PictureBackend(pub Arc<FilesRemote>);

/// The albums: none exist in the core yet.
pub struct AlbumBackend;

impl Backend for PictureBackend {
    fn list(&self, query: &Values) -> Result<Vec<Record>, BackendError> {
        only_find_inputs(query, FIND_KEYS)?;
        let query = query.clone();
        self.0.run(move |c| async move {
            let mut out = Vec::new();
            for found in find_all(&c, &query, Some("image/")).await? {
                if let Found::File(r) = found {
                    out.push(as_picture(&r));
                }
            }
            Ok(out)
        })
    }

    fn reconnect(&self) -> Result<(), BackendError> {
        self.0.reconnect()
    }

    fn supported(&self, command: &str) -> bool {
        is_supported("Picture", command)
    }

    fn change(&self, _record: &str, _edit: &Edit) -> Result<Outcome, BackendError> {
        // The description has no `change` command: renaming, moving and the rest are commands with their own inputs.
        Err(unsupported("change"))
    }

    fn run(
        &self,
        name: &str,
        record: Option<&str>,
        input: &Values,
    ) -> Result<Outcome, BackendError> {
        let input = with_record(input, "picture", record);
        if !is_supported("Picture", name) {
            return Err(unsupported(name));
        }
        let name = name.to_owned();
        match name.as_str() {
            "download" => self
                .0
                .run_within(self.0.limit_of(&name), move |c| async move {
                    download(&c.transfer, need_id(&input, "picture")?, &input).await
                }),
            "sensitiveMark" => {
                // The marking belongs to the file: the same call as `File.classify`.
                let mut as_file = input.clone();
                if let Some(id) = text(&input, "picture") {
                    as_file.insert("file".into(), vec![id.to_owned()]);
                }
                FileBackend(Arc::clone(&self.0)).run("classify", None, &as_file)
            }
            _ => Err(unsupported(&name)),
        }
    }
}

fn as_picture(file: &Record) -> Record {
    let mut v = Values::new();
    for key in ["name", "mediaType", "size", "sensitive"] {
        if let Some(value) = file.values.get(key) {
            v.insert(key.to_owned(), value.clone());
        }
    }
    v.insert("file".into(), vec![file.id.clone()]);
    Record {
        id: file.id.clone(),
        values: v,
    }
}

impl Backend for AlbumBackend {
    fn list(&self, _: &Values) -> Result<Vec<Record>, BackendError> {
        Ok(Vec::new())
    }

    fn change(&self, _record: &str, _edit: &Edit) -> Result<Outcome, BackendError> {
        Err(unsupported("change"))
    }

    fn run(&self, name: &str, _: Option<&str>, _: &Values) -> Result<Outcome, BackendError> {
        Err(unsupported(name))
    }
}
