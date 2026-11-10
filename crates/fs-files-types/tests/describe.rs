//! Tests of the type descriptions against the trait `DescribesType` and the registry.
//!
//! Pattern: none (tests).

use std::collections::BTreeSet;

use fs_files_types::{REQUIREMENTS, SUPPORTED, is_supported, registry, unsupported};
use fs_types::{BasicType, CommandKind};

#[test]
fn all_five_types_are_sound_and_registered() {
    let r = registry();
    let names: BTreeSet<&str> = r.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        BTreeSet::from(["Album", "File", "Folder", "Picture", "StorageBackend"])
    );
    for t in r.iter() {
        t.validate().expect("sound");
        assert_eq!(t.program, "fs-files");
    }
    assert_eq!(r.unresolved_targets(), []);
}

#[test]
fn the_standard_of_each_type_is_named() {
    let r = registry();
    let standard = |n: &str| r.get(n).unwrap().standard.clone();
    assert_eq!(standard("File"), "AS2 Document");
    assert_eq!(standard("Folder"), "AS2 Collection");
    assert_eq!(standard("Album"), "AS2 Collection");
    assert_eq!(standard("Picture"), "AS2 Image");
}

#[test]
fn every_requirement_names_a_described_command() {
    let r = registry();
    for q in REQUIREMENTS {
        let t = r.get(q.data_type).unwrap_or_else(|| panic!("{}", q.id));
        assert!(
            t.find_command(q.command).is_some(),
            "{} -> {}.{}",
            q.id,
            q.data_type,
            q.command
        );
    }
}

#[test]
fn every_described_command_is_a_requirement_or_a_core_command() {
    // the core's own commands that `requirements.md` has no id for
    let core = [
        ("File", "restoreTo"),
        ("File", "emptyTrash"),
        ("File", "uploadResume"),
        ("Picture", "sensitiveMark"),
        ("Album", "find"),
        ("Folder", "move"),
        ("Folder", "resolve"),
    ];
    let named: BTreeSet<(&str, &str)> = REQUIREMENTS
        .iter()
        .map(|q| (q.data_type, q.command))
        .chain(core)
        .collect();
    for t in registry().iter() {
        for c in &t.commands {
            assert!(
                named.contains(&(t.name.as_str(), c.name.as_str())),
                "{}.{}",
                t.name,
                c.name
            );
        }
    }
}

#[test]
fn the_support_table_names_only_described_commands_and_the_core_ones() {
    let r = registry();
    for (t, c) in SUPPORTED {
        assert!(
            r.get(t).and_then(|d| d.find_command(c)).is_some(),
            "{t}.{c}"
        );
    }
    // the methods of Files / Folders / Transfer / Backends the contract has, each reachable as a command
    for (t, c) in [
        ("Folder", "create"),
        ("Folder", "rename"),
        ("Folder", "move"),
        ("Folder", "delete"),
        ("Folder", "resolve"),
        ("Folder", "rescan"),
        ("File", "move"),
        ("File", "copy"),
        ("File", "rename"),
        ("File", "delete"),
        ("File", "restore"),
        ("File", "restoreTo"),
        ("File", "purge"),
        ("File", "emptyTrash"),
        ("File", "resolve"),
        ("File", "classify"),
        ("File", "find"),
        ("File", "upload"),
        ("File", "download"),
        ("File", "uploadList"),
        ("StorageBackend", "find"),
        ("StorageBackend", "create"),
        ("StorageBackend", "delete"),
    ] {
        assert!(is_supported(t, c), "{t}.{c}");
    }
    assert!(!is_supported("File", "preview"));
    assert!(!is_supported("Album", "create"));
    assert!(unsupported().len() > 50);
}

#[test]
fn a_change_or_delete_names_its_record_so_the_frontend_offers_it() {
    for t in registry().iter() {
        for c in &t.commands {
            if matches!(c.kind, CommandKind::Change | CommandKind::Delete) {
                assert!(
                    c.input
                        .iter()
                        .any(|f| f.basic == BasicType::Reference && f.target == t.name),
                    "{}.{} would not be offered",
                    t.name,
                    c.name
                );
            }
        }
    }
}

#[test]
fn each_type_has_a_find_that_lists_it() {
    for t in registry().iter() {
        let find = t
            .find_command("find")
            .unwrap_or_else(|| panic!("{}", t.name));
        assert_eq!(find.kind, CommandKind::Find);
        assert!(
            !find.input.iter().any(|f| f.target == t.name),
            "{}: find must not bind",
            t.name
        );
    }
}

#[test]
fn the_catalog_survives_the_wire() {
    use fs_interfaces::types_capnp::type_catalog::describe_results;
    let r = registry();
    let mut message = capnp::message::Builder::new_default();
    let root = message.init_root::<describe_results::Builder>();
    fs_files_types::write_catalog(&r, root.init_types(u32::try_from(r.len()).unwrap()));
    let read = fs_files_types::read_catalog(
        message
            .get_root_as_reader::<describe_results::Reader>()
            .unwrap()
            .get_types()
            .unwrap(),
    )
    .unwrap();
    let before: Vec<_> = r.iter().cloned().collect();
    assert_eq!(read, before);
}
