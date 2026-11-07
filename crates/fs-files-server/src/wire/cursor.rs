//! The cursor of a page on the wire.
//!
//! Pattern: Adapter — the core's `Cursor` (a place in the ordered list) as the opaque bytes of
//! `common.capnp`. The form is the one of `fs_interfaces::rpc::Page` (a version byte, then the place),
//! so a consumer sees one kind of cursor from every program. Before 1.0 it becomes a stable key.

use fs_files_core::Cursor;
use fs_interfaces::rpc::invalid_request;

const V1: u8 = 1;
/// No list this program holds has more entries; a place beyond is no cursor of ours, and as an offset it
/// could wrap to a negative one in the store, which reads as the start.
const LAST_PLACE: usize = u32::MAX as usize;

/// The cursor a consumer handed back; empty is the start.
pub(crate) fn read(opaque: &[u8]) -> capnp::Result<Option<Cursor>> {
    if opaque.is_empty() {
        return Ok(None);
    }
    let invalid = || invalid_request("invalid cursor");
    let (&version, rest) = opaque.split_first().ok_or_else(invalid)?;
    let bytes: [u8; 8] = rest.try_into().map_err(|_| invalid())?;
    if version != V1 {
        return Err(invalid());
    }
    let place = usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| invalid())?;
    if place > LAST_PLACE {
        return Err(invalid());
    }
    Ok(Some(Cursor::at(place)))
}

/// The bytes for the next page; empty at the end.
pub(crate) fn write(next: Option<Cursor>) -> Vec<u8> {
    next.map_or_else(Vec::new, |c| {
        let mut out = vec![V1];
        out.extend_from_slice(
            &u64::try_from(c.position())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        out
    })
}
