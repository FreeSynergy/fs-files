//! A download takes the name of the file from the server; a name that is a path must not decide where
//! the bytes go. Tests the adapter's download over a real server that tells such a name.

mod common;

use common::{Providers, Renamed, World};
use fs_components_workbenches::file_manager::FileSource;

fn world_telling(name: &str) -> (World, String) {
    let told = name.to_string();
    let w = World::behind(|repo, _| {
        let mut p = Providers::of(repo);
        p.files = std::sync::Arc::new(Renamed(repo.clone(), told));
        p
    });
    let made = w.file(None, "real.txt", b"bytes");
    (w, made.id.to_string())
}

#[test]
fn a_name_that_is_a_path_is_refused_and_nothing_is_written() {
    for told in ["../x", "/tmp/escape", "a/b", ".."] {
        let (w, key) = world_telling(told);
        let out = w.outside();
        let before: Vec<_> = std::fs::read_dir(&out).unwrap().collect();
        let err = w.adapter.download(&key, out.to_str().unwrap(), false);
        assert!(err.is_err(), "{told}: {err:?}");
        assert_eq!(
            std::fs::read_dir(&out).unwrap().count(),
            before.len(),
            "{told}"
        );
        assert!(!w.dir.path().join("x").exists(), "{told}");
    }
}

#[test]
fn an_honest_name_still_downloads() {
    let (w, key) = world_telling("fine.txt");
    let out = w.outside();
    w.adapter
        .download(&key, out.to_str().unwrap(), false)
        .unwrap();
    assert_eq!(std::fs::read(out.join("fine.txt")).unwrap(), b"bytes");
}
