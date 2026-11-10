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

/// Names Windows keeps for devices, with or without an extension (`NUL.txt` is the device too). `COM0` and `LPT0` count too, and
/// so do `COM¹`, `COM²`, `COM³` (and `LPT`): the console takes the superscript digits as the numbers they look like.
const DEVICES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

fn is_device(stem: &str) -> bool {
    if DEVICES.iter().any(|d| stem.eq_ignore_ascii_case(d)) {
        return true;
    }
    let mut chars = stem.chars();
    let head: String = chars.by_ref().take(3).collect();
    let (Some(last), None) = (chars.next(), chars.next()) else {
        return false;
    };
    (head.eq_ignore_ascii_case("COM") || head.eq_ignore_ascii_case("LPT"))
        && matches!(last, '0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}')
}

/// True for the short form Windows makes of a long name (`FS-FIL~1`, `PROGRA~1.TXT`): up to eight characters before the first dot
/// that end in `~` and digits. The short form of a reserved long name (`.fs-files`) reaches the same entry, so it is refused as
/// a whole; the cost is that a file really called `abc~1` cannot be kept on Windows.
fn is_short_form(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let Some((head, digits)) = stem.rsplit_once('~') else {
        return false;
    };
    stem.chars().count() <= 8
        && !head.is_empty()
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

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
        if is_device(stem) {
            return Some("is the name of a device on Windows (CON, NUL, COM1, ...)");
        }
        if is_short_form(name) {
            return Some(
                "looks like a short 8.3 name Windows makes (NAME~1), which can reach another entry",
            );
        }
        None
    }

    fn same_name(&self, a: &str, b: &str) -> bool {
        Self::folded(a) == Self::folded(b)
    }
}

impl WindowsNames {
    /// NTFS compares names after mapping every UTF-16 unit through its `$UpCase` table, which is a simple one-to-one upper-case
    /// mapping without context. `to_lowercase` is not that (`\u{3a3}` has a context rule, `\u{130}` becomes two characters), so
    /// each character is mapped by its upper case when that is a single character, and kept otherwise. The table of a volume is
    /// not available without a call into the system, so this is an approximation; the rare difference makes two names that
    /// NTFS keeps apart look like one (the safe side: a rename is then done by the careful way).
    fn folded(name: &str) -> String {
        name.chars()
            .map(|c| {
                let mut up = c.to_uppercase();
                match (up.next(), up.next()) {
                    (Some(u), None) => u,
                    _ => c,
                }
            })
            .collect()
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
            "a:b",
            "a<b",
            "x?",
            "name.",
            "name ",
            "NUL",
            "nul.txt",
            "Com3.log",
            "aux",
            "COM0",
            "lpt0.txt",
            "COM\u{b9}",
            "lpt\u{b3}.x",
            "FS-FIL~1",
            "fs-fi~12.txt",
        ] {
            assert!(rules.refuse(bad).is_some(), "{bad}");
        }
        for good in [
            "a.txt",
            "Foto (2).jpg",
            "console",
            "COM10",
            "COM\u{2074}",
            "a~b",
            "abcdefghi~1",
            "x~",
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
        assert!(rules.same_name("stra\u{df}e", "STRA\u{df}E"));
        assert!(!rules.same_name("a", "b"));
    }
}
