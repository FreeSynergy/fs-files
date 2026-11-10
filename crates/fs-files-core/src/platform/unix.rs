//! Pattern: Strategy (Unix: any name the shared rules accept, a flush of the folder, `renameat2` on Linux).

use std::io;
use std::path::Path;

use super::facade::Platform;
use super::{Durability, Moves, NameRules};

struct UnixNames;
struct UnixDurability;
struct UnixMoves;

impl NameRules for UnixNames {
    fn refuse(&self, _name: &str) -> Option<&'static str> {
        None
    }

    fn same_name(&self, a: &str, b: &str) -> bool {
        a == b
    }
}

impl Durability for UnixDurability {
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        std::fs::File::open(dir)?.sync_all()
    }
}

impl Moves for UnixMoves {
    #[cfg(target_os = "linux")]
    fn rename_new_native(&self, from: &Path, to: &Path, native: bool) -> io::Result<bool> {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        use rustix::io::Errno;
        if !native {
            return Ok(false);
        }
        match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
            Ok(()) => Ok(true),
            Err(e) if e == Errno::INVAL || e == Errno::NOSYS => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn rename_new_native(&self, _from: &Path, _to: &Path, _native: bool) -> io::Result<bool> {
        Ok(false)
    }
}

pub(super) fn platform() -> Platform {
    Platform {
        names: Box::new(UnixNames),
        durability: Box::new(UnixDurability),
        moves: Box::new(UnixMoves),
    }
}
