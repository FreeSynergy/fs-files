//! Trash count, emptying the trash and rescanning a folder.
//!
//! Pattern: Adapter — contract calls turned into the workbench's reports.

use fs_components_workbenches::file_manager::{EmptyReport, FileError, RescanSummary, TrashCount};
use fs_interfaces::common_capnp;
use fs_interfaces::files_capnp::{folders, search};
use fs_interfaces::rpc::{read_uuid, write_uuid};

use super::convert::{entry_of, file_of, optional};
use crate::remote::{FilesRemote, problem};

impl FilesRemote {
    pub(super) fn do_trash_count(&self) -> Result<TrashCount, FileError> {
        self.run(move |c| async move {
            let (mut files, mut cursor) = (0usize, Vec::<u8>::new());
            loop {
                let mut call = c.search.find_request();
                call.get().init_filter().set_trashed(true);
                call.get().init_cursor().set_opaque(&cursor);
                let answer = call.send().promise.await.map_err(|e| problem(&e))?;
                let page = answer
                    .get()
                    .and_then(search::find_results::Reader::get_page)
                    .map_err(|e| problem(&e))?;
                files += page.get_items().map_err(|e| problem(&e))?.len() as usize;
                let next = page
                    .get_next()
                    .and_then(common_capnp::cursor::Reader::get_opaque)
                    .map_err(|e| problem(&e))?;
                if next.is_empty() {
                    // Marked files are not told apart yet (ADR 0029): none is counted as marked.
                    return Ok(TrashCount::new(files, 0));
                }
                cursor = next.to_vec();
            }
        })
    }

    pub(super) fn do_empty_trash(&self) -> Result<EmptyReport, FileError> {
        self.run(move |c| async move {
            let answer = c
                .folders
                .empty_trash_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(folders::empty_trash_results::Reader::get_report)
                .map_err(|e| problem(&e))?;
            let mut failed: Vec<String> = Vec::new();
            for u in report.get_failed().map_err(|e| problem(&e))? {
                if let Some(u) = read_uuid(u) {
                    failed.push(u.to_string());
                }
            }
            if report.get_failed_missing() > 0 {
                failed.push(format!("{} more", report.get_failed_missing()));
            }
            Ok(EmptyReport::new(report.get_purged() as usize, failed))
        })
    }

    pub(super) fn do_rescan(&self, folder: Option<&str>) -> Result<RescanSummary, FileError> {
        let folder = optional(folder)?;
        self.run(move |c| async move {
            let mut call = c.folders.rescan_request();
            write_uuid(call.get().init_folder(), folder);
            let answer = call.send().promise.await.map_err(|e| problem(&e))?;
            let report = answer
                .get()
                .and_then(folders::rescan_results::Reader::get_report)
                .map_err(|e| problem(&e))?;
            let mut summary = RescanSummary::default();
            for item in report.get_added().map_err(|e| problem(&e))? {
                summary
                    .added
                    .push(entry_of(item).map_err(|e| problem(&e))?.name);
            }
            for f in report.get_updated().map_err(|e| problem(&e))? {
                summary
                    .updated
                    .push(file_of(f).map_err(|e| problem(&e))?.name);
            }
            for f in report.get_missing().map_err(|e| problem(&e))? {
                summary
                    .missing
                    .push(file_of(f).map_err(|e| problem(&e))?.name);
            }
            for s in report.get_skipped().map_err(|e| problem(&e))? {
                summary.skipped.push(
                    s.map_err(|e| problem(&e))?
                        .to_string()
                        .map_err(|e| FileError::failed(e.to_string()))?,
                );
            }
            Ok(summary)
        })
    }
}
