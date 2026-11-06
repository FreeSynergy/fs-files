//! A strict in-memory S3: it enforces what a real S3 enforces (parts of at least 5 MiB except the
//! last, at most 10 000 parts, ranges inside the object), so the backend's logic is tested without a server.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use fs_files_core::{Error, ObjectApi, ObjectEntry, Result};

#[derive(Default)]
pub struct Fake {
    objects: Mutex<HashMap<String, Vec<u8>>>,
    uploads: Mutex<HashMap<String, BTreeMap<usize, Vec<u8>>>>,
    pub multiparts_started: AtomicUsize,
    pub fail_deletes: AtomicBool,
}

fn err(m: &str) -> Error {
    Error::Backend(m.into())
}

impl Fake {
    pub fn keys(&self) -> Vec<String> {
        let mut k: Vec<_> = self.objects.lock().unwrap().keys().cloned().collect();
        k.sort();
        k
    }
    pub fn open_uploads(&self) -> usize {
        self.uploads.lock().unwrap().len()
    }
}

#[async_trait]
impl ObjectApi for Fake {
    async fn put(&self, key: &str, data: Vec<u8>) -> Result<()> {
        self.objects.lock().unwrap().insert(key.into(), data);
        Ok(())
    }
    async fn head(&self, key: &str) -> Result<Option<u64>> {
        Ok(self
            .objects
            .lock()
            .unwrap()
            .get(key)
            .map(|v| v.len() as u64))
    }
    async fn get_range(&self, key: &str, start: u64, end: u64) -> Result<Option<Vec<u8>>> {
        let objects = self.objects.lock().unwrap();
        let Some(v) = objects.get(key) else {
            return Ok(None);
        };
        if start > end || end > v.len() as u64 {
            return Err(err("416 InvalidRange"));
        }
        Ok(Some(v[start as usize..end as usize].to_vec()))
    }
    async fn delete(&self, key: &str) -> Result<()> {
        if self.fail_deletes.load(Ordering::SeqCst) {
            return Err(err("injected delete failure"));
        }
        self.objects.lock().unwrap().remove(key);
        Ok(())
    }
    async fn list(&self, prefix: &str) -> Result<Vec<ObjectEntry>> {
        let objects = self.objects.lock().unwrap();
        let mut found: Vec<_> = objects
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| ObjectEntry {
                key: k.clone(),
                size: v.len() as u64,
            })
            .collect();
        found.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(found)
    }
    async fn create_multipart(&self, key: &str) -> Result<String> {
        let n = self.multiparts_started.fetch_add(1, Ordering::SeqCst);
        let id = format!("upload-{n}");
        self.uploads
            .lock()
            .unwrap()
            .insert(format!("{key}#{id}"), BTreeMap::new());
        Ok(id)
    }
    async fn put_part(&self, key: &str, id: &str, index: usize, data: Vec<u8>) -> Result<String> {
        if index >= 10_000 {
            return Err(err("InvalidPart: more than 10000 parts"));
        }
        let mut uploads = self.uploads.lock().unwrap();
        let parts = uploads
            .get_mut(&format!("{key}#{id}"))
            .ok_or_else(|| err("NoSuchUpload"))?;
        let etag = format!("\"{}-{}\"", index, data.len());
        parts.insert(index, data);
        Ok(etag)
    }
    async fn complete(&self, key: &str, id: &str, etags: Vec<String>) -> Result<()> {
        let mut uploads = self.uploads.lock().unwrap();
        let parts = uploads
            .remove(&format!("{key}#{id}"))
            .ok_or_else(|| err("NoSuchUpload"))?;
        let mut whole = Vec::new();
        let last = etags.len().saturating_sub(1);
        for (i, etag) in etags.iter().enumerate() {
            let part = parts.get(&i).ok_or_else(|| err("InvalidPart: missing"))?;
            if *etag != format!("\"{}-{}\"", i, part.len()) {
                return Err(err("InvalidPart: etag mismatch"));
            }
            if i != last && part.len() < 5 << 20 {
                return Err(err("EntityTooSmall: a part but the last is under 5 MiB"));
            }
            whole.extend(part);
        }
        self.objects.lock().unwrap().insert(key.into(), whole);
        Ok(())
    }
    async fn abort(&self, key: &str, id: &str) -> Result<()> {
        self.uploads.lock().unwrap().remove(&format!("{key}#{id}"));
        Ok(())
    }
}
