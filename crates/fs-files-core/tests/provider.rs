//! The provider rule (ADR 0045): every provider has every method; what it does not offer answers
//! an explicit "not possible", and its report says the same.

mod common;

use std::sync::Arc;

use async_trait::async_trait;
use common::env;
use fs_files_core::{BackendInfo, Backends, Error, Files, Page, Result, Transfer};
use fs_interfaces::Capabilities;
use fs_store::ObjectId;

/// A read-only source, for example a mounted archive.
struct ReadOnly;

#[async_trait]
impl Files for ReadOnly {
    fn capabilities(&self) -> Capabilities {
        let mut c = Capabilities::new("read-only");
        c.supported("folder.list");
        c.unsupported("folder.create", "this source cannot be written");
        c
    }
    async fn get(&self, _: ObjectId) -> Result<Option<fs_files_core::File>> {
        Ok(None)
    }
    async fn get_folder(&self, _: ObjectId) -> Result<Option<fs_files_core::Folder>> {
        Ok(None)
    }
    async fn list(&self, _: Option<ObjectId>, _: Option<fs_files_core::Cursor>) -> Result<Page> {
        Ok(Page {
            items: vec![],
            next: None,
        })
    }
}

#[async_trait]
impl Backends for ReadOnly {
    async fn list_backends(&self) -> Result<Vec<BackendInfo>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn a_missing_method_says_not_possible() {
    let p: Arc<dyn Files> = Arc::new(ReadOnly);
    let Err(Error::Unsupported(u)) = p.create_folder(None, "x").await else {
        panic!("expected not possible")
    };
    assert_eq!((u.capability(), u.status()), ("folder.create", 501));
    assert!(!p.capabilities().is_supported("folder.create"));
    for r in [p.trash(ObjectId::new()).await, p.sharing().await] {
        assert!(matches!(r, Err(Error::Unsupported(_))));
    }
    let b: Arc<dyn Backends> = Arc::new(ReadOnly);
    assert!(matches!(
        b.add_backend("{}").await,
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        b.remove_backend("x").await,
        Err(Error::Unsupported(_))
    ));
}

#[tokio::test]
async fn sharing_gives_its_plain_reason_and_subscribe_is_offered() {
    let p: Arc<dyn Files> = Arc::new(env().await.repo);
    let Err(Error::Unsupported(s)) = p.sharing().await else {
        panic!()
    };
    assert_eq!(
        s.reason(),
        "one specification for all programs, not written yet"
    );
    let caps = p.capabilities();
    assert!(caps.is_supported("file.subscribe"));
    assert!(caps.why_not("share.create").is_some() && caps.why_not("file.versions").is_some());
    for id in [
        "file.upload",
        "file.download",
        "file.purge",
        "backend.add",
        "folder.delete",
        "file.classify",
    ] {
        assert!(caps.is_supported(id), "{id}");
    }
}

#[tokio::test]
async fn the_provider_traits_are_object_safe() {
    let env = env().await;
    let r = env.repo.clone();
    let _: (Arc<dyn Files>, Arc<dyn Transfer>, Arc<dyn Backends>) =
        (Arc::new(r.clone()), Arc::new(r.clone()), Arc::new(r));
}
