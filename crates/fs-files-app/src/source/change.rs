//! Changing the tree: create, rename, move, copy, trash, delete, restore, purge.
//!
//! Pattern: Adapter — one contract call per change, the conflict rule of the workbench turned into the contract's.

use fs_components_workbenches::file_manager::{
    EntryKind, EntryRef, FileError, FolderInfo, NameRule,
};
use fs_interfaces::files_capnp::folders;
use fs_interfaces::rpc::write_uuid;

use super::convert::{file_of, folder_of, id, on_conflict, optional};
use crate::remote::{FilesRemote, problem};

impl FilesRemote {
    pub(super) fn do_create_folder(
        &self,
        parent: Option<&str>,
        name: &str,
    ) -> Result<FolderInfo, FileError> {
        let (parent, name) = (optional(parent)?, name.to_string());
        self.run(move |c| async move {
            let mut call = c.folders.create_folder_request();
            write_uuid(call.get().init_parent(), parent);
            call.get().set_name(name.as_str());
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let e = answer
                .get()
                .and_then(folders::create_folder_results::Reader::get_folder)
                .and_then(folder_of)
                .map_err(|e| problem(&e))?;
            Ok(FolderInfo::new(e.key, e.name, e.folder))
        })
    }

    pub(super) fn do_rename(
        &self,
        entry: &EntryRef,
        name: &str,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, kind, name) = (id(&entry.key)?, entry.kind, name.to_string());
        self.run(move |c| async move {
            // The name it has now: under keep-both the server may have chosen another than asked.
            if kind == EntryKind::Folder {
                let mut call = c.folders.rename_folder_request();
                write_uuid(call.get().init_id(), Some(key));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(on_conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                answer
                    .get()
                    .and_then(folders::rename_folder_results::Reader::get_folder)
                    .and_then(folder_of)
                    .map(|f| f.name)
                    .map_err(|e| problem(&e))
            } else {
                let mut call = c.folders.rename_file_request();
                write_uuid(call.get().init_id(), Some(key));
                call.get().set_name(name.as_str());
                call.get().set_on_conflict(on_conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                answer
                    .get()
                    .and_then(folders::rename_file_results::Reader::get_file)
                    .and_then(file_of)
                    .map(|f| f.name)
                    .map_err(|e| problem(&e))
            }
        })
    }

    pub(super) fn do_move_to(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, kind, to) = (id(&entry.key)?, entry.kind, optional(folder)?);
        self.run(move |c| async move {
            if kind == EntryKind::Folder {
                let mut call = c.folders.move_folder_request();
                write_uuid(call.get().init_id(), Some(key));
                write_uuid(call.get().init_parent(), to);
                call.get().set_on_conflict(on_conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                answer
                    .get()
                    .and_then(folders::move_folder_results::Reader::get_folder)
                    .and_then(folder_of)
                    .map(|f| f.name)
                    .map_err(|e| problem(&e))
            } else {
                let mut call = c.folders.move_file_request();
                write_uuid(call.get().init_id(), Some(key));
                write_uuid(call.get().init_folder(), to);
                call.get().set_on_conflict(on_conflict(rule));
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                answer
                    .get()
                    .and_then(folders::move_file_results::Reader::get_file)
                    .and_then(file_of)
                    .map(|f| f.name)
                    .map_err(|e| problem(&e))
            }
        })
    }

    pub(super) fn do_copy_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, to) = (id(key)?, optional(folder)?);
        self.run(move |c| async move {
            let mut call = c.folders.copy_file_request();
            write_uuid(call.get().init_id(), Some(key));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(on_conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(folders::copy_file_results::Reader::get_file)
                .and_then(file_of)
                .map(|f| f.name)
                .map_err(|e| problem(&e))
        })
    }

    pub(super) fn do_trash(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.trash_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    pub(super) fn do_delete_folder(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.delete_folder_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }

    pub(super) fn do_restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.restore_request();
            write_uuid(call.get().init_id(), Some(key));
            call.get().set_on_conflict(on_conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(folders::restore_results::Reader::get_file)
                .and_then(file_of)
                .map(|f| f.name)
                .map_err(|e| problem(&e))
        })
    }

    pub(super) fn do_restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (key, to) = (id(key)?, optional(folder)?);
        self.run(move |c| async move {
            let mut call = c.folders.restore_to_request();
            write_uuid(call.get().init_id(), Some(key));
            write_uuid(call.get().init_folder(), to);
            call.get().set_on_conflict(on_conflict(rule));
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            answer
                .get()
                .and_then(folders::restore_to_results::Reader::get_file)
                .and_then(file_of)
                .map(|f| f.name)
                .map_err(|e| problem(&e))
        })
    }

    pub(super) fn do_purge(&self, key: &str) -> Result<(), FileError> {
        let key = id(key)?;
        self.run(move |c| async move {
            let mut call = c.folders.purge_request();
            write_uuid(call.get().init_id(), Some(key));
            call.send()
                .promise
                .await
                .map(|_| ())
                .map_err(|e| problem(&e))
        })
    }
}
