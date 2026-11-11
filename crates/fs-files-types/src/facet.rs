//! The facets of the files program: which data types a window of `fs-files --facet NAME` shows.
//!
//! Pattern: Registry (a fixed table) — the program is one (`fs-files`), the link `fs-gallery` is a facet of it (ADR 0063,
//! 0066). The server serves all types whatever the facet says; the facet is a view of the window.

/// The facets and the data types each one shows, in the order of the navigation.
pub const FACETS: &[(&str, &[&str])] = &[
    ("files", &["File", "Folder", "StorageBackend"]),
    ("gallery", &["Picture", "Album"]),
];

/// The data types of the facet `name`; `None` for a name that is no facet of this program.
pub fn facet_types(name: &str) -> Option<&'static [&'static str]> {
    FACETS.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// Check a facet name (already shaped by `fs_interfaces::rpc::parse_facet`) against the facets of this program.
///
/// # Errors
/// The text for the person: the name is unknown, the known ones are listed.
pub fn check_facet(name: &str) -> Result<(), String> {
    if facet_types(name).is_some() {
        return Ok(());
    }
    let known: Vec<&str> = FACETS.iter().map(|(n, _)| *n).collect();
    Err(format!(
        "unknown facet {name}: this program has {}",
        known.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_of_the_registry_is_in_exactly_one_facet() {
        for d in crate::registry().iter() {
            let n = FACETS
                .iter()
                .filter(|(_, types)| types.contains(&d.name.as_str()))
                .count();
            assert_eq!(n, 1, "{} is in {n} facets", d.name);
        }
    }

    #[test]
    fn only_known_names_pass() {
        assert!(check_facet("gallery").is_ok());
        assert!(check_facet("files").is_ok());
        assert!(check_facet("calendar").unwrap_err().contains("gallery"));
    }
}
