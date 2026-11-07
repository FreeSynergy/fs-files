//! Files, folders and pages of entries on the wire.
//!
//! Pattern: Adapter — `File`, `Folder`, `Entry` and `Page` between Rust and `files.capnp`. The
//! envelope is the object as stored. A page is cut by the core at 100 entries; here it is also held
//! to what a message carries.

use fs_files_core::{Entry, File, Folder, Page};
use fs_interfaces::files_capnp::{entry, entry_page, file, folder};
use fs_interfaces::rpc::{check_answer, envelope_weight, write_envelope};

use super::ids::write_optional;
use super::{count, cursor};

/// What the parts of a file or folder beyond its envelope weigh: ids, numbers, framing.
const FRAMING: usize = 128;

/// Write a file.
pub(crate) fn write_file(mut out: file::Builder<'_>, f: &File) {
    write_envelope(out.reborrow().init_envelope(), &f.envelope);
    out.set_media_type(f.media_type.as_str());
    out.set_size(f.size);
    out.set_checksum(f.checksum.as_str());
    write_optional(out.reborrow().init_folder(), f.folder);
    out.set_trashed(f.trashed);
}

/// Write a folder.
pub(crate) fn write_folder(mut out: folder::Builder<'_>, f: &Folder) {
    write_envelope(out.reborrow().init_envelope(), &f.envelope);
    write_optional(out.reborrow().init_parent(), f.parent);
}

/// Write an entry.
pub(crate) fn write_entry(out: entry::Builder<'_>, e: &Entry) {
    match e {
        Entry::File(f) => write_file(out.init_file(), f),
        Entry::Folder(f) => write_folder(out.init_folder(), f),
    }
}

/// What a file weighs in a message.
pub(crate) fn file_weight(f: &File) -> usize {
    FRAMING + envelope_weight(&f.envelope) + f.media_type.len() + f.checksum.len()
}

/// What an entry weighs in a message.
pub(crate) fn weight(e: &Entry) -> usize {
    match e {
        Entry::File(f) => file_weight(f),
        Entry::Folder(f) => FRAMING + envelope_weight(&f.envelope),
    }
}

/// Write a page of the core. One that would not fit a message is refused, not cut.
pub(crate) fn write_page(mut out: entry_page::Builder<'_>, page: &Page) -> capnp::Result<()> {
    check_answer(page.items.iter().map(weight).sum())?;
    let mut items = out.reborrow().init_items(count(page.items.len())?);
    for (i, e) in (0u32..).zip(&page.items) {
        write_entry(items.reborrow().get(i), e);
    }
    out.init_next().set_opaque(&cursor::write(page.next));
    Ok(())
}
