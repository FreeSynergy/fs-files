//! What kind of file it is: the first bytes, then the ending of the name, then whether it reads as text.
//!
//! Pattern: Chain of responsibility — each step answers or passes; nothing here leaves the machine and nothing
//! reads more than the first bytes. The answer is a media type; `application/octet-stream` only when nothing fits.

const UNKNOWN: &str = "application/octet-stream";

/// The endings of names that the magic bytes cannot tell (text, formats of plain text, some containers).
const BY_ENDING: &[(&str, &str)] = &[
    ("txt", "text/plain"),
    ("text", "text/plain"),
    ("log", "text/plain"),
    ("md", "text/markdown"),
    ("markdown", "text/markdown"),
    ("csv", "text/csv"),
    ("tsv", "text/tab-separated-values"),
    ("html", "text/html"),
    ("htm", "text/html"),
    ("css", "text/css"),
    ("js", "text/javascript"),
    ("mjs", "text/javascript"),
    ("json", "application/json"),
    ("xml", "application/xml"),
    ("yaml", "application/yaml"),
    ("yml", "application/yaml"),
    ("toml", "application/toml"),
    ("ini", "text/plain"),
    ("rs", "text/x-rust"),
    ("py", "text/x-python"),
    ("c", "text/x-c"),
    ("h", "text/x-c"),
    ("cpp", "text/x-c++"),
    ("java", "text/x-java"),
    ("go", "text/x-go"),
    ("ts", "text/x-typescript"),
    ("sh", "text/x-shellscript"),
    ("sql", "application/sql"),
    ("svg", "image/svg+xml"),
    ("ics", "text/calendar"),
    ("vcf", "text/vcard"),
    ("ods", "application/vnd.oasis.opendocument.spreadsheet"),
    ("odt", "application/vnd.oasis.opendocument.text"),
    ("odp", "application/vnd.oasis.opendocument.presentation"),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("tar", "application/x-tar"),
    ("gz", "application/gzip"),
    ("7z", "application/x-7z-compressed"),
    ("mp3", "audio/mpeg"),
    ("flac", "audio/flac"),
    ("ogg", "audio/ogg"),
    ("wav", "audio/wav"),
    ("mp4", "video/mp4"),
    ("mkv", "video/x-matroska"),
    ("webm", "video/webm"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

/// The media type of a file called `name` that starts with `head`.
pub(super) fn media_type(name: &str, head: &[u8]) -> String {
    if let Some(found) = infer::get(head) {
        // A container (zip) that the ending names more closely (docx, odt) is that more exact kind.
        let by_name = by_ending(name);
        return match by_name {
            Some(exact) if found.mime_type() == "application/zip" => exact.to_owned(),
            _ => found.mime_type().to_owned(),
        };
    }
    if let Some(known) = by_ending(name) {
        return known.to_owned();
    }
    if reads_as_text(head) {
        return "text/plain".to_owned();
    }
    UNKNOWN.to_owned()
}

fn by_ending(name: &str) -> Option<&'static str> {
    let ending = name.rsplit_once('.')?.1.to_ascii_lowercase();
    BY_ENDING
        .iter()
        .find(|(e, _)| *e == ending)
        .map(|(_, mime)| *mime)
}

/// No NUL and valid UTF-8 (a cut in the middle of a character at the end of the head is allowed).
fn reads_as_text(head: &[u8]) -> bool {
    if head.is_empty() || head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::media_type;

    #[test]
    fn magic_bytes_win_then_the_ending_then_text() {
        assert_eq!(media_type("x.bin", b"\x89PNG\r\n\x1a\n"), "image/png");
        assert_eq!(media_type("notes.md", b"# Notes"), "text/markdown");
        assert_eq!(media_type("README", b"hello\n"), "text/plain");
        assert_eq!(media_type("x.dat", &[0, 159, 146, 150]), "application/octet-stream");
        assert_eq!(media_type("empty.txt", b""), "text/plain");
    }

    #[test]
    fn a_zip_named_like_an_office_file_is_that_file() {
        let zip = b"PK\x03\x04\x14\x00\x00\x00\x08\x00";
        assert_eq!(media_type("a.zip", zip), "application/zip");
        assert!(media_type("a.docx", zip).contains("wordprocessingml"));
    }
}
