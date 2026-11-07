//! `Folders.get`: a folder can be resolved by id.

mod common;

use common::env;
use fs_files_core::Files;

#[tokio::test]
async fn a_folder_is_resolved_by_id() {
    let e = env().await;
    let r = &e.repo;
    assert!(r.capabilities().is_supported("folder.resolve"));
    let top = r.create_folder(None, "Urlaub").await.unwrap();
    let sub = r.create_folder(Some(top.id), "Strand").await.unwrap();
    assert_eq!(r.get_folder(top.id).await.unwrap(), Some(top.clone()));
    let found = r.get_folder(sub.id).await.unwrap().unwrap();
    assert_eq!(
        (found.name.as_str(), found.parent),
        ("Strand", Some(top.id))
    );
    // a file id is not a folder, an unknown id is nothing
    let f = common::upload(r, None, "a.txt", "text/plain", b"x").await;
    assert_eq!(r.get_folder(f.id).await.unwrap(), None);
    assert_eq!(
        r.get_folder(fs_objects::ObjectId::new()).await.unwrap(),
        None
    );
    // renaming shows in the resolved folder
    r.rename_folder(top.id, "Ferien").await.unwrap();
    assert_eq!(r.get_folder(top.id).await.unwrap().unwrap().name, "Ferien");
}
