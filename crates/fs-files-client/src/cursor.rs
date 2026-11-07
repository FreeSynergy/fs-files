//! The cursor of a page on the wire.
//!
//! Pattern: Adapter — the core's `Cursor` (a place in the ordered list) and the opaque bytes of
//! `common.capnp`, in the form of `fs_interfaces::rpc::Page` (a version byte, then the place) that the
//! server reads and writes. The client never makes up a place: it hands back what the server sent. The
//! core's type is a place, so the bytes are read into one; a form it does not know is a reply that
//! cannot be read, not a guess.

use fs_files_core::{Cursor, Error, Result};

const V1: u8 = 1;

/// The bytes of a cursor; empty is the start.
pub(crate) fn write(cursor: Option<Cursor>) -> Vec<u8> {
    cursor.map_or_else(Vec::new, |c| {
        let mut out = vec![V1];
        out.extend_from_slice(
            &u64::try_from(c.position())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        out
    })
}

/// The cursor of the next page; empty is the end.
pub(crate) fn read(opaque: &[u8]) -> Result<Option<Cursor>> {
    if opaque.is_empty() {
        return Ok(None);
    }
    let unknown = || {
        Error::Backend("files server: it sent a cursor of a form this client does not know".into())
    };
    let (&version, rest) = opaque.split_first().ok_or_else(unknown)?;
    let bytes: [u8; 8] = rest.try_into().map_err(|_| unknown())?;
    if version != V1 {
        return Err(unknown());
    }
    let place = usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| unknown())?;
    Ok(Some(Cursor::at(place)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_goes_there_and_back_and_the_end_is_empty() {
        assert_eq!(write(None), Vec::<u8>::new());
        assert_eq!(read(&[]).unwrap(), None);
        let c = Cursor::at(300);
        assert_eq!(read(&write(Some(c))).unwrap(), Some(c));
    }

    #[test]
    fn a_form_it_does_not_know_is_refused() {
        assert!(read(&[2, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(read(&[1, 0]).is_err());
    }
}
