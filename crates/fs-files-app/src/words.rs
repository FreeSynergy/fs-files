//! The words of the window (English), as the `Say` of the frontend.
//!
//! Pattern: Strategy (`Say`) — keys to texts (`coding.md` §2a: what a person reads is translatable; a catalog of another
//! language replaces this one). The labels of a description are keys such as `files.media-type` or
//! `file.cmd.rename-bulk`; a key without an entry reads as its last word, capitalised, so no label shows as a key.

use fs_components_fields::Say;

/// The English words.
pub struct English;

const KEYS: &[(&str, &str)] = &[
    ("action.add", "Add"),
    ("action.back", "Back"),
    ("action.cancel", "Cancel"),
    ("action.close", "Close"),
    ("action.remove", "Remove"),
    ("action.save", "Save"),
    ("menu.program", "Program"),
    ("menu.view", "View"),
    ("menu.help", "Help"),
    ("menu.quit", "Quit"),
    ("menu.about", "About"),
    ("message.done", "Done"),
    ("message.rejected", "The program refused this"),
    ("message.not-found", "Not found"),
    (
        "message.unavailable",
        "The files program cannot be reached or did not do it",
    ),
    ("message.unsaved", "There is text that was not sent"),
    ("problem.empty", "Needs a value"),
    ("problem.too-long", "Too long"),
    (
        "problem.character",
        "Contains a character that is not allowed",
    ),
    (
        "problem.choice.not-in-list",
        "Not possible: the name is taken or not one of the choices",
    ),
    ("problem.uri.bad", "Not a link"),
    (
        "problem.uri.not-absolute",
        "Needs a file: link, such as file:///home/me/photo.jpg",
    ),
    ("sort.ascending", "ascending"),
    ("sort.descending", "descending"),
    ("finding.slot", "Value {n}"),
    ("list.empty", "Nothing here"),
    ("list.separator", ", "),
    ("list.stale", "Not up to date"),
    ("detail.not-loaded", "Not loaded"),
    ("confirm.sure", "Are you sure?"),
    ("value.none", "none"),
    ("about.version", "Version"),
    ("about.origin", "Origin"),
    ("about.licence", "Licence"),
    ("about.repository", "Repository"),
    ("about.unoffered", "Not offered"),
    ("file", "Files"),
    ("folder", "Folders"),
    ("storage", "Storage"),
    ("album", "Albums"),
    ("picture", "Pictures"),
];

impl Say for English {
    fn say(&self, key: &str) -> String {
        if let Some((_, text)) = KEYS.iter().find(|(k, _)| *k == key) {
            return (*text).to_owned();
        }
        let word = key.rsplit('.').next().unwrap_or(key).replace('-', " ");
        let mut chars = word.chars();
        chars
            .next()
            .map(|f| f.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}
