//! Part of the window source: changing.
//!
//! Pattern: Adapter — see `mod.rs`.

use fs_components_workbenches::file_manager::{
    EmptyReport, EntryKind, EntryRef, FileError, FolderInfo, NameRule, RescanSummary,
};
use fs_interfaces::rpc::write_uuid;

use super::adapter::RemoteFileSource;
use super::convert::{conflict, entry_of, folder_id, id_of, new_name, text_name};
use crate::remote::problem;
use crate::values::{Found, entry_record, folder_record};

impl RemoteFileSource {
    pub(super) fn do_create_folder(
        &self,
        parent: Option<&str>,
        name: &str,
    ) -> Result<FolderInfo, FileError> {
        let (parent, name) = (folder_id(parent)?, name.to_owned());
        self.run(move |c| async move {
            let mut call = c.folders.create_folder_request();
            write_uuid(call.get().init_parent(), parent);
            call.get().set_name(name.as_str());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let f = answer
                .get()
                .and_then(|r| r.get_folder())
                .map_err(|e| problem(&e))?;
            let e = entry_of(Found::Folder(folder_record(f).map_err(|e| problem(&e))?));
            Ok(FolderInfo::new(e.key, e.name, e.folder))
        })
    }

    pub(super) fn do_rename(
        &self,
        entry: &EntryRef,
        name: &str,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, name, folder) = (
            id_of(&entry.key)?,
            name.to_owned(),
            entry.kind == EntryKind::Folder,
        );
        self.run(move |c| async move {
            if folder {
                let mut call = c.folders.rename_folder_request();
                write_uuid(call.get().init_id(), Some(id));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let f = answer
                    .get()
                    .and_then(|r| r.get_folder())
                    .map_err(|e| problem(&e))?;
                Ok(text_name(&entry_of(Found::Folder(
                    folder_record(f).map_err(|e| problem(&e))?,
                ))))
            } else {
                let mut call = c.folders.rename_file_request();
                write_uuid(call.get().init_id(), Some(id));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                new_name(
                    answer
                        .get()
                        .and_then(|r| r.get_file())
                        .map_err(|e| problem(&e))?,
                )
            }
        })
    }

    pub(super) fn do_move_to(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to, is_folder) = (
            id_of(&entry.key)?,
            folder_id(folder)?,
            entry.kind == EntryKind::Folder,
        );
        self.within("move", move |c| async move {
            if is_folder {
                let mut call = c.folders.move_folder_request();
                write_uuid(call.get().init_id(), Some(id));
                write_uuid(call.get().init_parent(), to);
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let f = answer
                    .get()
                    .and_then(|r| r.get_folder())
                    .map_err(|e| problem(&e))?;
                Ok(text_name(&entry_of(Found::Folder(
                    folder_record(f).map_err(|e| problem(&e))?,
                ))))
            } else {
                let mut call = c.folders.move_file_request();
                write_uuid(call.get().init_id(), Some(id));
                write_uuid(call.get().init_folder(), to);
                call.get().set_on_conflict(conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                new_name(
                    answer
                        .get()
                        .and_then(|r| r.get_file())
                        .map_err(|e| problem(&e))?,
                )
            }
        })
    }

    pub(super) fn do_copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to) = (id_of(key)?, folder_id(folder)?);
        self.within("copy", move |c| async move {
            let mut call = c.folders.copy_file_request();
            write_uuid(call.get().init_id(), Some(id));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(
                answer
                    .get()
                    .and_then(|r| r.get_file())
                    .map_err(|e| problem(&e))?,
            )
        })
    }

    pub(super) fn do_trash(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.trash_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    pub(super) fn do_delete_folder(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.delete_folder_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    pub(super) fn do_restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        let id = id_of(key)?;
        self.within("restore", move |c| async move {
            let mut call = c.folders.restore_request();
            write_uuid(call.get().init_id(), Some(id));
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(
                answer
                    .get()
                    .and_then(|r| r.get_file())
                    .map_err(|e| problem(&e))?,
            )
        })
    }

    pub(super) fn do_restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to) = (id_of(key)?, folder_id(folder)?);
        self.within("restoreTo", move |c| async move {
            let mut call = c.folders.restore_to_request();
            write_uuid(call.get().init_id(), Some(id));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            new_name(
                answer
                    .get()
                    .and_then(|r| r.get_file())
                    .map_err(|e| problem(&e))?,
            )
        })
    }

    pub(super) fn do_purge(&self, key: &str) -> Result<(), FileError> {
        let id = id_of(key)?;
        self.within("purge", move |c| async move {
            let mut call = c.folders.purge_request();
            write_uuid(call.get().init_id(), Some(id));
            call.send().promise.await.map_err(|e| problem(&e))?;
            Ok(())
        })
    }

    pub(super) fn do_empty_trash(&self) -> Result<EmptyReport, FileError> {
        self.within("emptyTrash", move |c| async move {
            let answer = c
                .folders
                .empty_trash_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(|r| r.get_report())
                .map_err(|e| problem(&e))?;
            let failed = report.get_failed().map_err(|e| problem(&e))?;
            let mut names: Vec<String> = failed
                .iter()
                .filter_map(|u| fs_interfaces::rpc::read_uuid(u))
                .map(|u| u.to_string())
                .collect();
            names.extend((0..report.get_failed_missing()).map(|_| String::from("?")));
            Ok(EmptyReport::new(report.get_purged() as usize, names))
        })
    }

    pub(super) fn do_rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        let folder = folder_id(folder)?;
        self.within("rescan", move |c| async move {
            let mut call = c.folders.rescan_request();
            write_uuid(call.get().init_folder(), folder);
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(|r| r.get_report())
                .map_err(|e| problem(&e))?;
            let mut out = RescanSummary::default();
            for e in report.get_added().map_err(|e| problem(&e))? {
                out.added.push(text_name(&entry_of(
                    entry_record(e).map_err(|e| problem(&e))?,
                )));
            }
            for f in report.get_updated().map_err(|e| problem(&e))? {
                out.updated.push(new_name(f)?);
            }
            for f in report.get_missing().map_err(|e| problem(&e))? {
                out.missing.push(new_name(f)?);
            }
            for s in report.get_skipped().map_err(|e| problem(&e))? {
                out.skipped.push(
                    s.map_err(|e| problem(&e))?
                        .to_string()
                        .map_err(|e| problem(&e.into()))?,
                );
            }
            Ok(out)
        })
    }
}
