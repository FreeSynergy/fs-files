//! Where things live inside the bucket.
//!
//! Pattern: Specification — one place that names every object the backend writes:
//! `<prefix>data/<key>` (committed), and for an upload in progress
//! `<prefix>pending/<key with / as ~>/state` plus `.../tail/<offset>` (bytes not yet in a part).

/// Object names for one backend (one bucket + optional prefix).
#[derive(Clone, Debug)]
pub(super) struct Layout {
    prefix: String,
}

impl Layout {
    /// `prefix` is empty or ends in `/`.
    pub(super) fn new(prefix: &str) -> Self {
        let p = prefix.trim_matches('/');
        Self {
            prefix: if p.is_empty() {
                String::new()
            } else {
                format!("{p}/")
            },
        }
    }

    pub(super) fn data(&self, key: &str) -> String {
        format!("{}data/{key}", self.prefix)
    }

    fn pending_dir(&self, key: &str) -> String {
        format!("{}pending/{}", self.prefix, key.replace('/', "~"))
    }

    pub(super) fn state(&self, key: &str) -> String {
        format!("{}/state", self.pending_dir(key))
    }

    pub(super) fn tail_dir(&self, key: &str) -> String {
        format!("{}/tail/", self.pending_dir(key))
    }

    /// Name of the tail piece that starts at `offset` (zero padded so names sort by offset).
    pub(super) fn tail(&self, key: &str, offset: u64) -> String {
        format!("{}{offset:020}", self.tail_dir(key))
    }
}
