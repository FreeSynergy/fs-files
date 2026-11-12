//! The names of the user folders of the person (Desktop, Downloads, Documents, Pictures, Music, Videos).
//!
//! Pattern: Adapter — the file `user-dirs.dirs` of the XDG user directories (`xdg-user-dirs`) says, in the language of the
//! person, what these folders are called (`XDG_PICTURES_DIR="$HOME/Bilder"`). The window pins the folders of its
//! storage that have such a name; the English names are the default where the file says nothing.

use std::path::PathBuf;

use fs_components_widgets::PinnedKind;

const KINDS: [(PinnedKind, &str, &str); 6] = [
    (PinnedKind::Desktop, "XDG_DESKTOP_DIR", "Desktop"),
    (PinnedKind::Downloads, "XDG_DOWNLOAD_DIR", "Downloads"),
    (PinnedKind::Documents, "XDG_DOCUMENTS_DIR", "Documents"),
    (PinnedKind::Pictures, "XDG_PICTURES_DIR", "Pictures"),
    (PinnedKind::Music, "XDG_MUSIC_DIR", "Music"),
    (PinnedKind::Videos, "XDG_VIDEOS_DIR", "Videos"),
];

/// The names the text of a `user-dirs.dirs` gives to the six kinds; a kind the text does not name has its English name.
pub(crate) fn names_in(text: &str) -> Vec<(PinnedKind, String)> {
    KINDS
        .iter()
        .map(|(kind, key, english)| {
            let found = text.lines().find_map(|line| {
                let value = line.trim().strip_prefix(key)?.strip_prefix('=')?;
                let path = value.trim().trim_matches('"');
                path.rsplit('/')
                    .find(|part| !part.is_empty())
                    .map(str::to_owned)
            });
            (*kind, found.unwrap_or_else(|| (*english).to_owned()))
        })
        .collect()
}

/// The names of this person's user folders, from the file of the system if there is one.
pub(crate) fn user_folder_names() -> Vec<(PinnedKind, String)> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    let text = config
        .map(|c| c.join("user-dirs.dirs"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    names_in(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_come_from_the_file_in_the_language_of_the_person_and_default_to_english() {
        let text = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Schreibtisch\"\nXDG_PICTURES_DIR=\"$HOME/Bilder/\"\nXDG_MUSIC_DIR=\"$HOME/Musik\"\n";
        let names = names_in(text);
        let get = |k: PinnedKind| names.iter().find(|(x, _)| *x == k).unwrap().1.clone();
        assert_eq!(get(PinnedKind::Desktop), "Schreibtisch");
        assert_eq!(get(PinnedKind::Pictures), "Bilder");
        assert_eq!(get(PinnedKind::Music), "Musik");
        assert_eq!(
            get(PinnedKind::Downloads),
            "Downloads",
            "not named: English"
        );
        assert_eq!(names_in("").len(), 6);
    }
}
