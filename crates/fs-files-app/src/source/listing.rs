//! Lists, one thing, folders, the start of a file.
//!
//! Pattern: Adapter — a request of the workbench as the contract's call. The contract's `find` has no
//! way to say "at the top level only", so a filter there drops what lies deeper: a page can be short.
//! Folders for the tree are found by listing by size, because a folder counts as 0 and the contract has
//! no filter by kind: the walk ends at the first file with bytes.

use chrono::{DateTime, NaiveDate, Utc};
use fs_components_workbenches::file_manager::{
    EntryPage, EntryRef, FileEntry, FileError, FolderInfo, ListFilter, ListRequest, PageMark,
    PreviewBytes, Scope, SortKey, SortOrder,
};
use fs_files_core::{Cursor, Entry, FileFilter, Page, Sort, SortBy};
use std::sync::Arc;

use super::adapter::{FileAdapter, entry_of_file, entry_of_folder, folder_of, id_of, info_of};
use crate::words::say;

/// The contract's order.
pub(super) fn sort_of(order: SortOrder) -> Sort {
    Sort {
        by: match order.key {
            SortKey::Modified => SortBy::Modified,
            SortKey::Size => SortBy::Size,
            SortKey::Type => SortBy::Type,
            _ => SortBy::Name,
        },
        descending: order.descending,
    }
}

fn day(text: &str) -> Result<DateTime<Utc>, FileError> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc())
        .ok_or_else(|| FileError::failed(say::BAD_DAY))
}

fn filter_of(filter: &ListFilter, term: &str) -> Result<FileFilter, FileError> {
    let at = |text: &Option<String>| text.as_deref().map(day).transpose();
    Ok(FileFilter {
        name: (!term.is_empty()).then(|| term.to_string()),
        min_size: filter.min_size,
        max_size: filter.max_size,
        modified_after: at(&filter.modified_from)?,
        modified_before: at(&filter.modified_before)?,
        ..FileFilter::default()
    })
}

fn entry_of(entry: &Entry) -> FileEntry {
    match entry {
        Entry::File(file) => entry_of_file(file),
        Entry::Folder(folder) => entry_of_folder(folder),
    }
}

fn parent_of(entry: &Entry) -> Option<fs_objects::ObjectId> {
    match entry {
        Entry::File(file) => file.folder,
        Entry::Folder(folder) => folder.parent,
    }
}

impl FileAdapter {
    pub(super) fn do_list(
        &self,
        request: &ListRequest,
        after: Option<&PageMark>,
    ) -> Result<EntryPage, FileError> {
        let cursor = after.and_then(|m| m.0.downcast_ref::<Cursor>()).copied();
        let sort = sort_of(request.sort);
        let mut only_top = false;
        let page: Page = match &request.scope {
            Scope::Folder(folder) if !request.filter.is_active() && request.term.is_empty() => {
                let folder = folder_of(folder.as_deref())?;
                self.wait(self.files.list_sorted(folder, cursor, sort))?
            }
            scope => {
                let mut filter = filter_of(&request.filter, &request.term)?;
                match scope {
                    Scope::Trash => filter.trashed = true,
                    Scope::Folder(Some(key)) => filter.folder = Some(id_of(key)?),
                    Scope::Folder(None) => only_top = true,
                    _ => {}
                }
                self.wait(self.files.find_sorted(&filter, cursor, sort))?
            }
        };
        let entries = page
            .items
            .iter()
            .filter(|item| !only_top || parent_of(item).is_none())
            .map(entry_of)
            .collect();
        let next = page.next.map(|c| PageMark(Arc::new(c)));
        Ok(EntryPage::new(entries, next))
    }

    pub(super) fn do_entry(&self, reference: &EntryRef) -> Result<Option<FileEntry>, FileError> {
        let id = id_of(&reference.key)?;
        if reference.kind == fs_components_workbenches::file_manager::EntryKind::Folder {
            return Ok(self
                .wait(self.files.get_folder(id))?
                .as_ref()
                .map(entry_of_folder));
        }
        Ok(self.wait(self.files.get(id))?.as_ref().map(entry_of_file))
    }

    pub(super) fn do_folder(&self, key: &str) -> Result<Option<FolderInfo>, FileError> {
        let id = id_of(key)?;
        Ok(self.wait(self.files.get_folder(id))?.as_ref().map(info_of))
    }

    pub(super) fn do_subfolders(
        &self,
        parent: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<FolderInfo>, bool), FileError> {
        let parent = folder_of(parent)?;
        let sort = Sort {
            by: SortBy::Size,
            descending: false,
        };
        let mut found = Vec::new();
        let mut cursor = None;
        loop {
            let page = self.wait(self.files.list_sorted(parent, cursor, sort))?;
            for item in &page.items {
                match item {
                    Entry::Folder(_) if found.len() == limit => return Ok((found, true)),
                    Entry::Folder(folder) => found.push(info_of(folder)),
                    Entry::File(file) if file.size > 0 => return Ok((found, false)),
                    Entry::File(_) => {}
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => return Ok((found, false)),
            }
        }
    }

    pub(super) fn do_preview(&self, key: &str, limit: usize) -> Result<PreviewBytes, FileError> {
        let id = id_of(key)?;
        let blob = self.wait(self.transfer.open(id))?;
        let (total, _) = self.wait(blob.size())?;
        let wanted = u32::try_from(limit).unwrap_or(u32::MAX);
        let mut bytes = Vec::new();
        while bytes.len() < limit {
            let room = wanted - u32::try_from(bytes.len()).unwrap_or(wanted);
            let part = self.wait(blob.read(
                bytes.len() as u64,
                room.min(u32::try_from(fs_files_core::CHUNK).unwrap_or(u32::MAX)),
            ))?;
            if part.is_empty() {
                break;
            }
            bytes.extend(part);
        }
        Ok(PreviewBytes::new(bytes, total))
    }
}
