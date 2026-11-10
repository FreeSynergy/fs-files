//! `Files.types` over the wire: the catalog of the type descriptions is served and equals the registry.

mod common;

use common::with_server;
use fs_files_types::{read_catalog, registry};

#[tokio::test(flavor = "current_thread")]
async fn the_catalog_is_served_over_the_socket() {
    with_server(|root, _fx| async move {
        let catalog = root
            .types_request()
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_catalog()
            .unwrap();
        let answer = catalog.describe_request().send().promise.await.unwrap();
        let read = read_catalog(answer.get().unwrap().get_types().unwrap()).unwrap();
        let local: Vec<_> = registry().iter().cloned().collect();
        assert_eq!(read, local);
        assert!(read.iter().any(|t| t.name == "File" && t.find_command("find").is_some()));
    })
    .await;
}
