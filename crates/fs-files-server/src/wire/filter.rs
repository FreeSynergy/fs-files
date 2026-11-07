//! The search filter on the wire.
//!
//! Pattern: Adapter — `FileFilter` of `files.capnp` to the core's. An empty text and an unset
//! timestamp mean "no condition"; `maxSize` of -1 means no limit, any other negative value is refused.

use fs_files_core::FileFilter;
use fs_interfaces::common_capnp::timestamp;
use fs_interfaces::files_capnp::file_filter;
use fs_interfaces::rpc::{from_millis, invalid_request};

use super::ids::optional;
use super::text::{MAX_TERM, limited};

fn some(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_string())
}

/// Read a filter.
pub(crate) fn read(r: file_filter::Reader<'_>) -> capnp::Result<FileFilter> {
    let max = r.get_max_size();
    if max < -1 {
        return Err(invalid_request("maxSize must be -1 (no limit) or a size"));
    }
    // The readers borrow the message, not this call: the helper wants one lifetime for all four.
    let at = |present: bool, get: capnp::Result<timestamp::Reader<'_>>| {
        if !present {
            return Ok(None);
        }
        from_millis(get?.get_unix_millis()).map(Some)
    };
    Ok(FileFilter {
        name: some(limited(r.get_name()?, "name", MAX_TERM)?),
        media_type: some(limited(r.get_media_type()?, "mediaType", MAX_TERM)?),
        folder: optional(r.get_folder()?)?,
        min_size: (r.get_min_size() > 0).then_some(r.get_min_size()),
        max_size: u64::try_from(max).ok(),
        modified_after: at(r.has_modified_after(), r.get_modified_after())?,
        modified_before: at(r.has_modified_before(), r.get_modified_before())?,
        created_after: at(r.has_created_after(), r.get_created_after())?,
        created_before: at(r.has_created_before(), r.get_created_before())?,
        trashed: r.get_trashed(),
    })
}
