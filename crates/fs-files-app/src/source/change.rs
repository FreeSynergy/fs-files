//! The changes: names, places, the trash.
//!
//! Pattern: Adapter — each is one call of the contract with the rule for a taken name; there is no rule
//! that replaces. Folders have no copy and no trash in the contract, so neither is offered for them.

use fs_components_workbenches::file_manager::{
    EmptyReport, EntryKind, EntryRef, FileError, FolderInfo, NameRule, RescanSummary, TrashCount,
};
use fs_files_core::{Entry, File, FileFilter, OnConflict};

use super::adapter::{FileAdapter, folder_of, id_of, info_of};
use crate::words::{cli, fill, labels};

pub(super) fn rule_of(rule: NameRule) -> OnConflict {
    if rule == NameRule::KeepBoth {
        OnConflict::KeepBoth
    } else {
        OnConflict::Refuse
    }
}

/// A file as a report names it: a marked one says so (ADR 0029).
fn named(file: &File) -> String {
    if file.sensitive.is_empty() {
        file.name.clone()
    } else {
        fill(
            cli::MARKED_NAME,
            &[("mark", &labels().marked), ("name", &file.name)],
        )
    }
}

impl FileAdapter {
    pub(super) fn do_create_folder(
        &self,
        parent: Option<&str>,
        name: &str,
    ) -> Result<FolderInfo, FileError> {
        let parent = folder_of(parent)?;
        Ok(info_of(&self.wait(self.files.create_folder(parent, name))?))
    }

    pub(super) fn do_rename(
        &self,
        entry: &EntryRef,
        name: &str,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, on) = (id_of(&entry.key)?, rule_of(rule));
        if entry.kind == EntryKind::Folder {
            return Ok(self.wait(self.files.rename_folder_as(id, name, on))?.name);
        }
        Ok(self.wait(self.files.rename_file_as(id, name, on))?.name)
    }

    pub(super) fn do_move(
        &self,
        entry: &EntryRef,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to, on) = (id_of(&entry.key)?, folder_of(folder)?, rule_of(rule));
        if entry.kind == EntryKind::Folder {
            return Ok(self.wait(self.files.move_folder_as(id, to, on))?.name);
        }
        Ok(self.wait(self.files.move_file_as(id, to, on))?.name)
    }

    pub(super) fn do_copy(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to, on) = (id_of(key)?, folder_of(folder)?, rule_of(rule));
        Ok(self.wait(self.files.copy_file_as(id, to, on))?.name)
    }

    pub(super) fn do_restore(&self, key: &str, rule: NameRule) -> Result<String, FileError> {
        let id = id_of(key)?;
        Ok(self.wait(self.files.restore_as(id, rule_of(rule)))?.name)
    }

    pub(super) fn do_restore_to(
        &self,
        key: &str,
        folder: Option<&str>,
        rule: NameRule,
    ) -> Result<String, FileError> {
        let (id, to) = (id_of(key)?, folder_of(folder)?);
        Ok(self
            .wait(self.files.restore_to(id, to, rule_of(rule)))?
            .name)
    }

    /// Everything in the trash is counted: the contract has no count, and the person is told the number.
    pub(super) fn do_trash_count(&self) -> Result<TrashCount, FileError> {
        let filter = FileFilter {
            trashed: true,
            ..FileFilter::default()
        };
        let (mut files, mut marked) = (0, 0);
        let mut cursor = None;
        loop {
            let page = self.wait(self.files.find(&filter, cursor))?;
            for item in &page.items {
                if let Entry::File(file) = item {
                    files += 1;
                    marked += usize::from(!file.sensitive.is_empty());
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => return Ok(TrashCount::new(files, marked)),
            }
        }
    }

    pub(super) fn do_empty_trash(&self) -> Result<EmptyReport, FileError> {
        let report = self.wait(self.files.empty_trash())?;
        let failed = report
            .failed
            .iter()
            .map(|id| match self.wait(self.files.get(*id)) {
                Ok(Some(file)) => named(&file),
                _ => id.to_string(),
            })
            .collect();
        Ok(EmptyReport::new(report.purged as usize, failed))
    }

    pub(super) fn do_rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        let folder = folder_of(folder)?;
        let report = self.wait(self.files.rescan(folder))?;
        let mut summary = RescanSummary::default();
        summary.added = report
            .added
            .iter()
            .map(|e| match e {
                Entry::File(file) => file.name.clone(),
                Entry::Folder(folder) => folder.name.clone(),
            })
            .collect();
        summary.updated = report.updated.iter().map(named).collect();
        summary.missing = report.missing.iter().map(named).collect();
        summary.skipped = report.skipped;
        Ok(summary)
    }
}
