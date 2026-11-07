//! Reports and lists that can be longer than a message.
//!
//! Pattern: Adapter + Specification — `RescanReport`, `TrashReport`, `UploadInfo` and `BackendInfo`
//! between Rust and `files.capnp`, each held to a budget in bytes. A report that is cut says so in
//! `skipped` (rescan), and what is left out was still done: it is only not listed. The lists end
//! with what fits; nothing is truncated without a trace except where `DESIGN.md` says so.

use fs_files_core::{BackendInfo, RescanReport, TrashReport, UploadInfo};
use fs_interfaces::files_capnp::{backend_info, rescan_report, trash_report, upload_info};
use fs_interfaces::rpc::{MAX_PAGE_BYTES, capped, to_millis, write_uuid};

use super::count;
use super::entry::{file_weight, weight, write_entry, write_file};
use super::ids::write_optional;

/// Room kept for the lines that say a list was cut.
const NOTES: usize = 4096;
const TEXT_BYTES: usize = 16;
const ID_BYTES: usize = 24;
const UPLOAD_BYTES: usize = 160;

/// How many of `items` fit `budget`, taking what they use from it.
fn fitting<T>(items: &[T], budget: &mut usize, weight: impl Fn(&T) -> usize) -> usize {
    let mut kept = 0;
    for item in items {
        let w = weight(item);
        if w > *budget {
            break;
        }
        *budget -= w;
        kept += 1;
    }
    kept
}

/// Write a rescan report within a message. The entries left out are named in `skipped`.
pub(crate) fn write_rescan(
    mut out: rescan_report::Builder<'_>,
    report: &RescanReport,
) -> capnp::Result<()> {
    let mut budget = MAX_PAGE_BYTES - NOTES;
    let added = fitting(&report.added, &mut budget, weight);
    let updated = fitting(&report.updated, &mut budget, file_weight);
    let missing = fitting(&report.missing, &mut budget, file_weight);
    let mut notes = Vec::new();
    for (what, all, shown) in [
        (
            "added entries (they are taken over)",
            report.added.len(),
            added,
        ),
        (
            "updated files (they are updated)",
            report.updated.len(),
            updated,
        ),
        (
            "missing files (their records are kept)",
            report.missing.len(),
            missing,
        ),
    ] {
        if all > shown {
            notes.push(format!(
                "... {} more {what} are not listed here",
                all - shown
            ));
        }
    }
    let kept = fitting(&report.skipped, &mut budget, |t| {
        capped(t).len() + TEXT_BYTES
    });
    let mut skipped: Vec<String> = report.skipped[..kept]
        .iter()
        .map(|t| capped(t).to_string())
        .collect();
    if kept < report.skipped.len() {
        skipped.push(format!("... {} more", report.skipped.len() - kept));
    }
    skipped.extend(notes);

    let mut list = out.reborrow().init_added(count(added)?);
    for (i, e) in (0u32..).zip(&report.added[..added]) {
        write_entry(list.reborrow().get(i), e);
    }
    let mut list = out.reborrow().init_updated(count(updated)?);
    for (i, f) in (0u32..).zip(&report.updated[..updated]) {
        write_file(list.reborrow().get(i), f);
    }
    let mut list = out.reborrow().init_missing(count(missing)?);
    for (i, f) in (0u32..).zip(&report.missing[..missing]) {
        write_file(list.reborrow().get(i), f);
    }
    let mut list = out.init_skipped(count(skipped.len())?);
    for (i, t) in (0u32..).zip(&skipped) {
        list.set(i, t.as_str());
    }
    Ok(())
}

/// Write what `emptyTrash` did. The failed ids are at most what a message holds.
pub(crate) fn write_trash(
    mut out: trash_report::Builder<'_>,
    report: &TrashReport,
) -> capnp::Result<()> {
    out.set_purged(report.purged);
    let shown = report.failed.len().min(MAX_PAGE_BYTES / ID_BYTES);
    let mut list = out.init_failed(count(shown)?);
    for (i, id) in (0u32..).zip(&report.failed[..shown]) {
        write_uuid(list.reborrow().get(i), Some(id.as_uuid()));
    }
    Ok(())
}

/// Write one unfinished upload.
fn write_upload(mut out: upload_info::Builder<'_>, u: &UploadInfo) {
    out.set_id(u.id.as_str());
    write_optional(out.reborrow().init_folder(), u.folder);
    out.set_name(u.name.as_str());
    out.set_media_type(u.media_type.as_str());
    out.set_received(u.received);
    out.init_started().set_unix_millis(to_millis(u.started));
}

/// Write the unfinished uploads, oldest first, as many as fit a message. A consumer that deals with
/// these and asks again gets the rest.
pub(crate) fn write_uploads(
    out: capnp::struct_list::Builder<'_, upload_info::Owned>,
    items: &[UploadInfo],
    shown: usize,
) {
    let mut out = out;
    for (i, u) in (0u32..).zip(&items[..shown]) {
        write_upload(out.reborrow().get(i), u);
    }
}

/// How many of the uploads fit a message.
pub(crate) fn uploads_fitting(items: &[UploadInfo]) -> usize {
    let mut budget = MAX_PAGE_BYTES;
    fitting(items, &mut budget, |u| {
        UPLOAD_BYTES + u.id.len() + u.name.len() + u.media_type.len()
    })
}

/// Write one backend.
pub(crate) fn write_backend(mut out: backend_info::Builder<'_>, b: &BackendInfo) {
    out.set_id(b.id.as_str());
    out.set_kind(b.kind.as_str());
    out.set_active(b.active);
}
