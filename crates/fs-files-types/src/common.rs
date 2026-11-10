//! Small builders shared by the descriptions.
//!
//! Pattern: Builder (`coding.md` §2) — thin helpers over the `fs-types` builders, so each type file reads as a list of
//! commands. Labels are translation keys (`coding.md` §2a): `<type>.<field>` and `<type>.cmd.<command>`.

use fs_types::{BasicType, CommandDescription, CommandKind, FieldDescription};

/// The program that owns the types (ADR 0050); the gallery is a facet of it.
pub(crate) const PROGRAM: &str = "fs-files";

/// What to do when the target name is taken: the keys of `OnConflict` in `files.capnp`.
pub(crate) fn on_conflict() -> FieldDescription {
    FieldDescription::new("onConflict", BasicType::Choice)
        .choices(&["refuse", "keep-both"])
        .label("files.on-conflict")
}

/// A required reference to a record of `target`, named `name`.
pub(crate) fn record(name: &str, target: &str) -> FieldDescription {
    FieldDescription::new(name, BasicType::Reference)
        .required()
        .target(target)
        .label(&label(name))
}

/// An optional reference to a record of `target`.
pub(crate) fn optional_record(name: &str, target: &str) -> FieldDescription {
    FieldDescription::new(name, BasicType::Reference)
        .target(target)
        .label(&label(name))
}

/// A text field `name`.
pub(crate) fn text(name: &str) -> FieldDescription {
    FieldDescription::new(name, BasicType::Text).label(&label(name))
}

/// A field of `basic` with the shared label `files.<name>`.
pub(crate) fn field(name: &str, basic: BasicType) -> FieldDescription {
    FieldDescription::new(name, basic).label(&label(name))
}

/// A command `name` of `kind` on type key `scope` (`file`, `folder` …).
pub(crate) fn command(scope: &str, name: &str, kind: CommandKind) -> CommandDescription {
    CommandDescription::new(name, kind, &format!("{scope}.cmd.{}", kebab(name)))
}

/// A command that works on one record: its input starts with the reference to `target` named `bound`.
pub(crate) fn on_record(
    scope: &str,
    name: &str,
    kind: CommandKind,
    bound: &str,
    target: &str,
) -> CommandDescription {
    command(scope, name, kind).input(record(bound, target))
}

/// The search filter shared by the `find` commands of the file types.
pub(crate) fn filter_fields() -> Vec<FieldDescription> {
    vec![
        text("name"),
        text("mediaType"),
        optional_record("folder", "Folder"),
        field("minSize", BasicType::Text),
        field("maxSize", BasicType::Text),
        field("modifiedAfter", BasicType::DateTime),
        field("modifiedBefore", BasicType::DateTime),
        field("createdAfter", BasicType::DateTime),
        field("createdBefore", BasicType::DateTime),
        FieldDescription::new("sortBy", BasicType::Choice)
            .choices(&["name", "modified", "size", "media-type"])
            .label("files.sort-by"),
        FieldDescription::new("order", BasicType::Choice)
            .choices(&["ascending", "descending"])
            .label("files.order"),
    ]
}

/// `mediaType` -> `media-type`: translation keys are lower-case words.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn label(name: &str) -> String {
    format!("files.{}", kebab(name))
}
