//! The descriptions on the wire of `files.capnp` (`Files.types`).
//!
//! Pattern: Adapter between the plain structs of `fs-types` (v0.0.2, the version `fs-admin-frontend` uses) and the
//! generated types of the contract tag this program builds on. The two generated copies of `types_capnp` are different
//! Rust types of the same wire layout, so the kinds are converted by their number. Unknown kinds read as `unknown`.

use capnp::struct_list;
use fs_interfaces::types_capnp as w;
use fs_types as t;

fn len(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn write_field(f: &t::FieldDescription, mut b: w::field_description::Builder<'_>) {
    b.set_name(&f.name);
    b.set_basic(w::BasicType::try_from(f.basic as u16).unwrap_or(w::BasicType::Unknown));
    b.set_required(f.required);
    b.set_multiple(f.multiple);
    b.set_label(&f.label);
    b.set_target(&f.target);
    let mut list = b.init_choices(len(f.choices.len()));
    for (i, c) in (0..).zip(&f.choices) {
        list.set(i, c.as_str());
    }
}

fn write_fields(
    fields: &[t::FieldDescription],
    mut list: struct_list::Builder<'_, w::field_description::Owned>,
) {
    for (i, f) in (0..).zip(fields) {
        write_field(f, list.reborrow().get(i));
    }
}

/// Write the descriptions of a registry as the answer of `TypeCatalog.describe`.
pub fn write_catalog(
    registry: &t::TypeRegistry,
    mut list: struct_list::Builder<'_, w::type_description::Owned>,
) {
    for (i, d) in (0..).zip(registry.iter()) {
        let mut b = list.reborrow().get(i);
        b.set_name(&d.name);
        b.set_program(&d.program);
        b.set_standard(&d.standard);
        b.set_label(&d.label);
        write_fields(&d.fields, b.reborrow().init_fields(len(d.fields.len())));
        let mut cmds = b.init_commands(len(d.commands.len()));
        for (j, c) in (0..).zip(&d.commands) {
            let mut cb = cmds.reborrow().get(j);
            cb.set_name(&c.name);
            cb.set_kind(w::CommandKind::try_from(c.kind as u16).unwrap_or(w::CommandKind::Unknown));
            cb.set_label(&c.label);
            write_fields(&c.input, cb.reborrow().init_input(len(c.input.len())));
            write_fields(&c.result, cb.init_result(len(c.result.len())));
        }
    }
}

fn read_field(r: w::field_description::Reader<'_>) -> capnp::Result<t::FieldDescription> {
    let mut choices = Vec::new();
    for c in r.get_choices()? {
        choices.push(c?.to_string()?);
    }
    let basic = r.get_basic().map_or(0, |b| b as u16);
    Ok(t::FieldDescription {
        name: r.get_name()?.to_string()?,
        basic: t::BasicType::try_from(basic).unwrap_or(t::BasicType::Unknown),
        required: r.get_required(),
        multiple: r.get_multiple(),
        label: r.get_label()?.to_string()?,
        target: r.get_target()?.to_string()?,
        choices,
    })
}

fn read_fields(
    list: struct_list::Reader<'_, w::field_description::Owned>,
) -> capnp::Result<Vec<t::FieldDescription>> {
    list.iter().map(read_field).collect()
}

/// Read the descriptions of an answer of `TypeCatalog.describe`.
///
/// # Errors
/// A damaged message or a text that is not UTF-8.
pub fn read_catalog(
    list: struct_list::Reader<'_, w::type_description::Owned>,
) -> capnp::Result<Vec<t::TypeDescription>> {
    let mut out = Vec::new();
    for d in list {
        let mut commands = Vec::new();
        for c in d.get_commands()? {
            let kind = c.get_kind().map_or(0, |k| k as u16);
            commands.push(t::CommandDescription {
                name: c.get_name()?.to_string()?,
                kind: t::CommandKind::try_from(kind).unwrap_or(t::CommandKind::Unknown),
                label: c.get_label()?.to_string()?,
                input: read_fields(c.get_input()?)?,
                result: read_fields(c.get_result()?)?,
            });
        }
        out.push(t::TypeDescription {
            name: d.get_name()?.to_string()?,
            program: d.get_program()?.to_string()?,
            standard: d.get_standard()?.to_string()?,
            label: d.get_label()?.to_string()?,
            fields: read_fields(d.get_fields()?)?,
            commands,
        });
    }
    Ok(out)
}
