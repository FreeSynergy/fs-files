//! The type descriptions the server reports (`Files.types`).
//!
//! Pattern: Adapter — the catalog capability of the contract read into the plain descriptions of `fs-types`.

use fs_files_types::read_catalog;
use fs_types::TypeDescription;

use crate::remote::{FilesRemote, problem};

impl FilesRemote {
    /// What the server says about the data types it owns.
    ///
    /// # Errors
    /// The text for the person when the server cannot answer.
    pub fn describe(&self) -> Result<Vec<TypeDescription>, String> {
        self.run_within(self.start_limit(), |c| async move {
            let answer = c
                .files
                .types_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let catalog = answer
                .get()
                .and_then(|r| r.get_catalog())
                .map_err(|e| problem(&e))?;
            let answer = catalog
                .describe_request()
                .send()
                .promise
                .await
                .map_err(|e| problem(&e))?;
            let list = answer
                .get()
                .and_then(|r| r.get_types())
                .map_err(|e| problem(&e))?;
            read_catalog(list).map_err(|e| problem(&e))
        })
        .map_err(|e| format!("{e:?}"))
    }
}
