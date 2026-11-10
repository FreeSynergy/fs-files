//! Pattern: Strategy (Windows: names Windows cannot keep are refused by us, because some are not refused by the disk (`a:b` is a
//! stream of `a`); NTFS journals its metadata, so a rename is durable without a flush of the folder; there is no "rename, never
//! replace" step in the standard library, so the caller uses the hard link way).

use std::io;
use std::path::Path;

use super::facade::Platform;
use super::{Durability, Moves, NameRules};

struct WindowsNames;
struct WindowsDurability;
struct WindowsMoves;

/// Names Windows keeps for devices, with or without an extension (`NUL.txt` is the device too).
const DEVICES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

impl NameRules for WindowsNames {
    fn refuse(&self, name: &str) -> Option<&'static str> {
        if name
            .chars()
            .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        {
            return Some("contains a character Windows does not keep in a name (< > : \" | ? *)");
        }
        if name.ends_with(['.', ' ']) {
            return Some("ends with a dot or a space, which Windows takes away");
        }
        let stem = name.split('.').next().unwrap_or(name).trim_end();
        if DEVICES.iter().any(|d| stem.eq_ignore_ascii_case(d)) {
            return Some("is the name of a device on Windows (CON, NUL, COM1, ...)");
        }
        None
    }

    fn same_name(&self, a: &str, b: &str) -> bool {
        Self::lower(a) == Self::lower(b)
    }
}

impl WindowsNames {
    fn lower(name: &str) -> String {
        name.to_lowercase()
    }
}

impl Durability for WindowsDurability {
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        // A folder cannot be flushed on Windows; NTFS writes its metadata through its journal.
        Ok(())
    }
}

impl Moves for WindowsMoves {
    fn rename_new_native(&self, _from: &Path, _to: &Path, _native: bool) -> io::Result<bool> {
        Ok(false)
    }
}

pub(super) fn platform() -> Platform {
    Platform {
        names: Box::new(WindowsNames),
        durability: Box::new(WindowsDurability),
        moves: Box::new(WindowsMoves),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_windows_cannot_keep_is_refused() {
        let rules = WindowsNames;
        for bad in [
            "a:b", "a<b", "x?", "name.", "name ", "NUL", "nul.txt", "Com3.log", "aux",
        ] {
            assert!(rules.refuse(bad).is_some(), "{bad}");
        }
        for good in [
            "a.txt",
            "Foto (2).jpg",
            "console",
            "COM10",
            "nul-1",
            "bericht.final.pdf",
        ] {
            assert!(rules.refuse(good).is_none(), "{good}");
        }
    }

    #[test]
    fn windows_keeps_one_name_for_every_case() {
        let rules = WindowsNames;
        assert!(rules.same_name("Other", "OTHER"));
        assert!(!rules.same_name("a", "b"));
    }
}
