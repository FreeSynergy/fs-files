//! Finding single objects and answering the questions the rules need.
//!
//! Pattern: Repository (read side) — backend-neutral `Query` values; filtering is done by the store.

use fs_store::{ObjectId, Query, StoredObject};

use super::build::folder_key;
use super::consts::CONTEXT;
use super::handle::FileRepo;
use super::names::{copy_name, numbered_name};
use crate::error::{Error, Result};
use crate::model::{FolderRecord, OnConflict};

fn is_kind(o: &StoredObject, kind: &str) -> bool {
    o.index
        .get("kind")
        .is_some_and(|v| v.iter().any(|k| k == kind))
}

impl FileRepo {
    /// The stored object of a file (trashed or not).
    pub(super) async fn file_obj(&self, id: ObjectId) -> Result<StoredObject> {
        match self.objects.get(id).await? {
            Some(o) if is_kind(&o, "file") => Ok(o),
            _ => Err(Error::NotFound(format!("file {id}"))),
        }
    }

    /// The stored object of a folder.
    pub(super) async fn folder_obj(&self, id: ObjectId) -> Result<StoredObject> {
        match self.objects.get(id).await? {
            Some(o) if is_kind(&o, "folder") => Ok(o),
            _ => Err(Error::NotFound(format!("folder {id}"))),
        }
    }

    /// A file that is not in the trash (most changes need that).
    pub(super) async fn live_file_obj(&self, id: ObjectId) -> Result<StoredObject> {
        let o = self.file_obj(id).await?;
        if crate::model::FileRecord::of(&o)?.trashed {
            return Err(Error::Conflict(format!("file {id} is in the trash")));
        }
        Ok(o)
    }

    /// Fail unless `folder` is the top level or an existing folder.
    pub(super) async fn require_folder(&self, folder: Option<ObjectId>) -> Result<()> {
        if let Some(f) = folder {
            self.folder_obj(f).await?;
        }
        Ok(())
    }

    /// Fail if a live entry called `name` (exactly; the disk may know better) is already in `folder`.
    pub(super) async fn require_free(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        except: Option<ObjectId>,
    ) -> Result<()> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("folder", folder_key(folder))
            .with_index("trashed", "false")
            .with_index("name_exact", name)
            .limit(2);
        let taken = self.reads.query(&q).await?;
        if taken.iter().any(|o| Some(o.envelope.id) != except) {
            return Err(Error::name_taken(format!(
                "the name {name:?} is taken in this folder"
            )));
        }
        Ok(())
    }

    /// Entries of any kind (live or trashed) directly in `folder`.
    /// The name to use in `folder`: `name` if free, else by the rule `on`: refuse, or the first free numbered name.
    pub(super) async fn resolve_name(
        &self,
        folder: Option<ObjectId>,
        name: &str,
        except: Option<ObjectId>,
        on: OnConflict,
        copy: bool,
    ) -> Result<String> {
        let first = self.require_free(folder, name, except).await;
        match first {
            Err(Error::Conflict(_)) if on == OnConflict::KeepBoth => {}
            other => return other.map(|()| name.to_string()),
        }
        for n in 2..1000 {
            let candidate = if copy {
                copy_name(name, n - 1)
            } else {
                numbered_name(name, n)
            };
            if self.require_free(folder, &candidate, except).await.is_ok() {
                return Ok(candidate);
            }
        }
        Err(Error::Conflict("no free name to keep both".into()))
    }

    pub(super) async fn count_in(&self, folder: ObjectId) -> Result<usize> {
        let q = Query::new()
            .in_context(CONTEXT)
            .with_index("folder", folder.to_string());
        Ok(self.reads.count(&q).await?)
    }

    /// Fail if `target` is `id` or lies below it (a folder cannot go into itself).
    pub(super) async fn require_not_inside(
        &self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> Result<()> {
        let mut at = target;
        while let Some(f) = at {
            if f == id {
                return Err(Error::Conflict(
                    "a folder cannot be moved into itself or below itself".into(),
                ));
            }
            at = FolderRecord::of(&self.folder_obj(f).await?)?.parent;
        }
        Ok(())
    }

    /// Store an object under the actor.
    pub(super) async fn save(&self, o: StoredObject) -> Result<()> {
        self.objects.put(o, &self.actor).await?;
        Ok(())
    }
}
