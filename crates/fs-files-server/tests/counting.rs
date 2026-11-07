//! Reading all pages must cost a number of rows that grows with the data, not with its square. The
//! count is of rows the store returns, not of seconds: a fixed time says nothing on another machine.
//! The store is behind a counting decorator under the provider; the pages are read over the socket.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use common::search::find;
use common::{caps, list_all, with_repo};
use fs_files_core::{FileRepo, LocalFolder, Transfer};
use fs_store::traits::{ObjectQuery, ObjectRepository};
use fs_store::{Activity, LogEntry, ObjectId, Query, Result, Store, StoredObject, TursoStore};

/// Passes everything on and counts the objects the queries hand back.
struct Counting {
    store: Arc<dyn Store>,
    rows: AtomicUsize,
}

#[async_trait]
impl ObjectRepository for Counting {
    async fn put(&self, object: StoredObject, actor: &str) -> Result<Activity> {
        self.store.put(object, actor).await
    }
    async fn put_and_log(
        &self,
        object: StoredObject,
        actor: &str,
        extra: Vec<LogEntry>,
    ) -> Result<Vec<Activity>> {
        self.store.put_and_log(object, actor, extra).await
    }
    async fn get(&self, id: ObjectId) -> Result<Option<StoredObject>> {
        self.store.get(id).await
    }
    async fn delete(&self, id: ObjectId, actor: &str) -> Result<Activity> {
        self.store.delete(id, actor).await
    }
}

#[async_trait]
impl ObjectQuery for Counting {
    async fn query(&self, query: &Query) -> Result<Vec<StoredObject>> {
        let found = self.store.query(query).await?;
        self.rows.fetch_add(found.len(), Ordering::Relaxed);
        Ok(found)
    }
    async fn count(&self, query: &Query) -> Result<usize> {
        self.store.count(query).await
    }
}

/// Follow `pendingUploads` to the end; how many uploads came.
async fn pending_all(c: &common::Caps) -> usize {
    let (mut seen, mut cursor) = (0, vec![]);
    loop {
        let mut call = c.transfer.pending_uploads_request();
        call.get().init_cursor().set_opaque(&cursor);
        let answer = call.send().promise.await.unwrap();
        let r = answer.get().unwrap();
        seen += r.get_items().unwrap().len() as usize;
        cursor = r.get_next().unwrap().get_opaque().unwrap().to_vec();
        if cursor.is_empty() {
            return seen;
        }
    }
}

#[derive(Clone, Copy)]
enum Read {
    List,
    Find,
    Uploads,
}

/// Names that sort differently raw and folded: upper and lower case, umlauts, accents.
fn name_of(i: usize, n: usize) -> String {
    let lead = ["a", "A", "ä", "Ä", "z", "Z", "é", "ß"][i % 8];
    format!("{lead}-{:05}.txt", (i * 7919) % n)
}

/// Rows read to page through `n` entries with `how`, and how many entries came out.
async fn rows_for(n: usize, how: Read) -> (usize, usize) {
    let dir = tempfile::tempdir().unwrap();
    let counting = Arc::new(Counting {
        store: Arc::new(TursoStore::open_in_memory().await.unwrap()),
        rows: AtomicUsize::new(0),
    });
    let backend = Arc::new(LocalFolder::open(dir.path().join("files")).await.unwrap());
    let repo = Arc::new(
        FileRepo::with_backend(
            counting.clone(),
            counting.clone(),
            common::ME,
            "main",
            backend,
        )
        .with_page_size(8),
    );
    for i in 0..n {
        let name = name_of(i, n);
        let (upload, _) = repo.start_upload(None, &name, "text/plain").await.unwrap();
        if !matches!(how, Read::Uploads) {
            upload.write(0, b"x").await.unwrap();
            upload.finish().await.unwrap();
        }
    }
    counting.rows.store(0, Ordering::Relaxed);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let slot = seen.clone();
    with_repo(repo, |root| async move {
        let c = caps(&root).await;
        slot.set(match how {
            Read::List => list_all(&c, None).await.len(),
            Read::Find => find(&c, |_| {}, None).await.unwrap().len(),
            Read::Uploads => pending_all(&c).await,
        });
    })
    .await;
    (counting.rows.load(Ordering::Relaxed), seen.get())
}

#[tokio::test(flavor = "current_thread")]
async fn reading_all_pages_costs_rows_in_proportion_to_the_data() {
    for (name, how, small_n) in [
        ("list", Read::List, 80),
        ("find", Read::Find, 80),
        ("pendingUploads", Read::Uploads, 300),
    ] {
        let large_n = 2 * small_n;
        let (small, seen_small) = rows_for(small_n, how).await;
        let (large, seen_large) = rows_for(large_n, how).await;
        assert_eq!(
            (seen_small, seen_large),
            (small_n, large_n),
            "{name}: every entry once"
        );
        #[allow(clippy::cast_precision_loss)]
        let ratio = large as f64 / small as f64;
        eprintln!(
            "{name}: {small} rows for {small_n}, {large} rows for {large_n}, ratio {ratio:.2}"
        );
        // Linear is about 2, a whole list read for every page is about 4.
        assert!(
            ratio < 3.0,
            "{name}: {small} -> {large} rows, ratio {ratio:.2}"
        );
    }
}
