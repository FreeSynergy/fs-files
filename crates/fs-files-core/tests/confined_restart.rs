//! `backend.add` after a restart (nothing is mounted) and when an add fails: a backend that is not
//! mounted is still found on the disk, and a failed add leaves nothing it made.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use common::{ME, env, upload};
use fs_files_core::{
    Backends, BlobBackend, BlobInfo, ConfinedLocalFactory, Error, FileRepo, LocalFolder, Place,
    Result,
};
use fs_interfaces::Capabilities;
use fs_objects::{Store, TursoStore};

fn cfg(id: &str, path: &str, active: bool) -> String {
    serde_json::json!({ "id": id, "kind": "local", "path": path, "active": active }).to_string()
}

/// A repository on `store`, as after a restart: the main backend only.
async fn start(store: &Arc<dyn Store>, root: &Path) -> (FileRepo, PathBuf) {
    let base = root.join(".fs-files/backends");
    let main = Arc::new(LocalFolder::open(root).await.unwrap());
    let repo = FileRepo::new(store.clone(), ME, "main", main).with_local_base(&base);
    (repo, base)
}

async fn store() -> Arc<dyn Store> {
    Arc::new(TursoStore::open_in_memory().await.unwrap())
}

#[tokio::test]
async fn a_parent_of_a_backend_that_is_not_mounted_is_refused() {
    let e = env().await;
    let root = e.dir.path().join("root");
    let store = store().await;
    let (first, base) = start(&store, &root).await;
    first.add_backend(&cfg("b", "A/B", true)).await.unwrap();
    upload(&first, None, "kept.txt", "text/plain", b"b's bytes").await;
    first
        .add_backend(&cfg("deep", "A/C/D/E", false))
        .await
        .unwrap();

    let (again, _) = start(&store, &root).await; // a restart: nothing is mounted
    let err = again.add_backend(&cfg("a", "A", false)).await.unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");
    let err = again
        .add_backend(&cfg("c", "A/C", false))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");
    assert_eq!(
        std::fs::read(base.join("A/B/kept.txt")).unwrap(),
        b"b's bytes"
    );
    // The backend itself can come back under its id, and so can a sibling.
    again.add_backend(&cfg("b", "A/B", true)).await.unwrap();
    again
        .add_backend(&cfg("other", "A/B2", false))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_tree_too_deep_or_too_big_to_look_through_is_refused_not_trusted() {
    let e = env().await;
    let (repo, base) = start(&store().await, &e.dir.path().join("root")).await;
    let mut deep = base.join("Deep");
    for _ in 0..20 {
        deep.push("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    let err = repo
        .add_backend(&cfg("x", "Deep", false))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");

    std::fs::create_dir_all(base.join("Big")).unwrap();
    for n in 0..10_001 {
        std::fs::write(base.join(format!("Big/f{n}")), "").unwrap();
    }
    let err = repo.add_backend(&cfg("y", "Big", false)).await.unwrap_err();
    assert!(matches!(err, Error::Config(_)), "{err}");
    // A small ordinary folder with files is fine, and nothing was added to the big ones.
    std::fs::create_dir_all(base.join("Small/sub")).unwrap();
    std::fs::write(base.join("Small/sub/f"), "x").unwrap();
    repo.add_backend(&cfg("z", "Small", false)).await.unwrap();
    assert!(!base.join("Big/.fs-files").exists() && !base.join("Deep/.fs-files").exists());
}

/// A factory whose backend fails its check, after the real one made its folders.
struct FailsItsCheck(ConfinedLocalFactory);

struct CannotWrite(Arc<dyn BlobBackend>);

#[async_trait]
impl BlobBackend for CannotWrite {
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn kind(&self) -> &'static str {
        "local"
    }
    async fn get(&self, key: &str, offset: u64, length: u32) -> Result<Vec<u8>> {
        self.0.get(key, offset, length).await
    }
    async fn stat(&self, key: &str) -> Result<Option<BlobInfo>> {
        self.0.stat(key).await
    }
}

#[async_trait]
impl fs_files_core::BackendFactory for FailsItsCheck {
    fn kind(&self) -> &'static str {
        "fails-its-check"
    }
    async fn place(&self, config: &serde_json::Value) -> Result<Option<Place>> {
        self.0.place(config).await
    }
    fn forget(&self, place: &Place) {
        self.0.forget(place);
    }
    async fn build(&self, config: &serde_json::Value) -> Result<Arc<dyn BlobBackend>> {
        Ok(Arc::new(CannotWrite(self.0.build(config).await?)))
    }
}

fn failing(id: &str, path: &str) -> String {
    serde_json::json!({ "id": id, "kind": "fails-its-check", "path": path }).to_string()
}

fn tree(dir: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            out.push(e.path().strip_prefix(dir).unwrap().display().to_string());
            if e.file_type().unwrap().is_dir() {
                todo.push(e.path());
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn an_add_that_fails_its_check_leaves_only_what_was_there_before() {
    let e = env().await;
    let (repo, base) = start(&store().await, &e.dir.path().join("root")).await;
    let repo = repo.with_factory(Arc::new(FailsItsCheck(ConfinedLocalFactory::new(&base))));
    // An outer folder with something in it, an empty folder, and a new path in a new folder.
    std::fs::create_dir_all(base.join("Outer")).unwrap();
    std::fs::write(base.join("Outer/mine.txt"), "mine").unwrap();
    std::fs::create_dir_all(base.join("Empty")).unwrap();
    let before = tree(&base);

    for path in ["Outer/New/Deeper", "Empty", "Brand/New"] {
        let err = repo.add_backend(&failing("f", path)).await.unwrap_err();
        assert!(matches!(err, Error::Config(_)), "{path}: {err}");
        assert_eq!(tree(&base), before, "{path} left something behind");
    }
    assert_eq!(repo.list_backends().await.unwrap().len(), 1);
    assert_eq!(std::fs::read(base.join("Outer/mine.txt")).unwrap(), b"mine");
}

#[tokio::test]
async fn an_add_whose_folder_cannot_be_opened_leaves_the_folder_as_it_was() {
    use std::os::unix::fs::PermissionsExt;
    let e = env().await;
    let (repo, base) = start(&store().await, &e.dir.path().join("root")).await;
    let locked = base.join("Locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::write(locked.join("probe"), "").is_ok(); // running as root ignores modes
    let result = repo.add_backend(&cfg("l", "Locked", false)).await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    if !writable {
        assert!(result.is_err());
    }
    assert!(
        !locked.join(".fs-files").exists(),
        "nothing of ours is left in it"
    );
}

#[test]
fn forget_does_not_clean_through_a_link_that_took_the_place_of_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (elsewhere, base) = (dir.path().join("elsewhere"), dir.path().join("base"));
    std::fs::create_dir_all(elsewhere.join(".fs-files/pending")).unwrap();
    std::fs::create_dir_all(&base).unwrap();
    let link = base.join("X");
    std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
    let place = Place {
        path: link.clone(),
        made: vec![link.clone()],
        marker_new: true,
    };
    fs_files_core::BackendFactory::forget(&ConfinedLocalFactory::new(&base), &place);
    assert!(
        elsewhere.join(".fs-files/pending").is_dir(),
        "the other folder was touched"
    );
    assert!(link.exists(), "and so was the link");
}
