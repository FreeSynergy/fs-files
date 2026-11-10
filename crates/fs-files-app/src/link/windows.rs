//! Pattern: Strategy (Windows: `/C:/Users/me/a.jpg` is `C:\Users\me\a.jpg`; a path without a drive stays relative to the drive
//! of the program, which is what `file:///Users/me` means there).

use std::path::PathBuf;

use super::FileLinks;

pub(super) struct DrivePaths;

impl FileLinks for DrivePaths {
    fn path_of(&self, text: &str) -> Option<PathBuf> {
        let bytes = text.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':';
        let path = if drive { &text[1..] } else { text };
        Some(PathBuf::from(path.replace('/', "\\")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive_letter_follows_the_first_slash() {
        let links = DrivePaths;
        assert_eq!(
            links.path_of("/C:/Users/me/a b.jpg"),
            Some(PathBuf::from(r"C:\Users\me\a b.jpg"))
        );
        assert_eq!(
            links.path_of("/Users/me/a.jpg"),
            Some(PathBuf::from(r"\Users\me\a.jpg"))
        );
    }
}
