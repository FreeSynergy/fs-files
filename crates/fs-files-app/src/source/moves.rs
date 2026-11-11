//! Part of the window source: uploads and downloads.
//!
//! Pattern: Adapter — see `mod.rs`.

use fs_admin_frontend::Values;
use fs_components_workbenches::file_manager::{FileError, NameRule, PendingUpload};
use fs_interfaces::rpc::write_uuid;
use uuid::Uuid;

use super::adapter::RemoteFileSource;
use super::convert::{bare, id_of, link_of, new_name};
use crate::remote::problem;

impl RemoteFileSource {
    pub(super) fn do_upload(
        &self,
        path: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let mut input = Values::from([("source".to_owned(), vec![link_of(path)])]);
        if let Some(f) = folder {
            input.insert("folder".into(), vec![f.to_owned()]);
        }
        if rule == NameRule::KeepBoth {
            input.insert("onConflict".into(), vec!["keep-both".into()]);
        }
        self.within("upload", move |c| async move {
            let out = crate::transfer::upload(&c.transfer, &input).await?;
            let id = out.id.unwrap_or_default();
            // the name the file has now
            let mut call = c.folders.get_request();
            write_uuid(call.get().init_id(), Uuid::parse_str(&bare(&id)).ok());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(
                answer
                    .get()
                    .and_then(|r| r.get_file())
                    .map_err(|e| problem(&e))?,
            )
        })
    }

    pub(super) fn do_pending_uploads(&self) -> Result<Vec<PendingUpload>, FileError> {
        self.run(move |c| async move {
            let out = crate::uploads::pending(&c.transfer).await?;
            let col = |k: &str| out.values.get(k).cloned().unwrap_or_default();
            let (ids, names, got) = (col("uploadId"), col("name"), col("received"));
            Ok(ids
                .into_iter()
                .zip(names)
                .zip(got)
                .map(|((id, name), n)| PendingUpload::new(id, name, n.parse().unwrap_or(0)))
                .collect())
        })
    }

    pub(super) fn do_drop_upload(&self, id: &str) -> Result<(), FileError> {
        let input = Values::from([("uploadId".to_owned(), vec![id.to_owned()])]);
        self.run(move |c| async move {
            crate::uploads::abort(&c.transfer, &input).await?;
            Ok(())
        })
    }

    pub(super) fn do_download(&self, key: &str, path: &str) -> Result<String, FileError> {
        let id = id_of(key)?;
        let input = Values::from([("target".to_owned(), vec![link_of(path)])]);
        let target = path.to_owned();
        self.within("download", move |c| async move {
            crate::transfer::download(&c.transfer, id, &input).await?;
            Ok(target)
        })
    }
}
