//! Where the files may live relative to the program's own data: never around the database.

use std::path::Path;

use fs_files_server::check_places;

fn fresh() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir(&data).unwrap();
    (dir, data)
}

fn refused(root: &Path, data: &Path) -> String {
    check_places(root, data).expect_err("must be refused")
}

#[test]
fn a_root_that_holds_the_data_is_refused_in_every_shape() {
    let (dir, data) = fresh();
    assert!(refused(&data, &data).contains("objects.db"));
    assert!(refused(dir.path(), &data).contains("objects.db"));
    assert!(refused(&dir.path().join("data/./"), &data).contains("objects.db"));
    refused(&dir.path().join("data/../data"), &data);
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&data, &link).unwrap();
    refused(&link, &data);
    refused(&link.join("not-made-yet"), &data.join("not-made-yet"));
    refused(Path::new("/"), &data);
}

#[test]
fn a_root_beside_or_below_the_data_is_fine_made_or_not() {
    let (dir, data) = fresh();
    for ok in [
        data.join("files"),
        data.join("files/deeper"),
        dir.path().join("elsewhere"),
        dir.path().join("data-but-not-it"),
    ] {
        check_places(&ok, &data).unwrap_or_else(|e| panic!("{}: {e}", ok.display()));
    }
    let link = dir.path().join("to-elsewhere");
    std::fs::create_dir(dir.path().join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("elsewhere"), &link).unwrap();
    check_places(&link.join("new"), &data).unwrap();
}

#[test]
fn a_dot_dot_after_a_part_that_is_not_there_yet_does_not_hide_the_data() {
    let (dir, data) = fresh();
    for root in [
        dir.path().join("not-made/.."),
        data.join("files/.."),
        data.join("a/b/../.."),
        dir.path().join("not-made/../data/files/.."),
    ] {
        check_places(&root, &data).expect_err(&root.display().to_string());
    }
    assert!(
        !dir.path().join("not-made").exists(),
        "the check makes nothing"
    );
}
