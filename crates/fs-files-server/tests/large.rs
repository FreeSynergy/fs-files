//! A file larger than a message: it goes in and out in pieces, and no answer is bigger than a message allows.
#![allow(clippy::cast_possible_truncation)] // tests: small known test sizes, truncation cannot happen

mod common;

use common::{caps, checksum, read_file, with_server};
use fs_interfaces::rpc::MAX_PAGE_BYTES;

const MIB: usize = 1024 * 1024;

/// Bytes that are not periodic at any size a piece could have.
fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(31) ^ (i >> 7)) as u8)
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn a_file_of_34_mib_goes_in_by_chunks_and_out_by_clamped_reads() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let all = pattern(34 * MIB);
        let (upload, _) = common::start(&c, None, "large.bin").await.unwrap();
        for (i, chunk) in all.chunks(8 * MIB).enumerate() {
            let mut write = upload.write_request();
            write.get().set_offset((i * 8 * MIB) as u64);
            write.get().set_data(chunk);
            write.send().promise.await.unwrap();
        }
        let done = upload.finish_request().send().promise.await.unwrap();
        let file = read_file(done.get().unwrap().get_file().unwrap());
        assert_eq!(
            (file.size, file.checksum.clone()),
            (all.len() as u64, checksum(&all))
        );
        assert_eq!(
            std::fs::metadata(fx.root.join("large.bin")).unwrap().len(),
            all.len() as u64
        );

        let mut open = c.transfer.open_request();
        common::set_id(open.get().init_id(), file.id);
        let blob = open
            .send()
            .promise
            .await
            .unwrap()
            .get()
            .unwrap()
            .get_blob()
            .unwrap();
        let mut ask = blob.read_request();
        ask.get().set_offset(0);
        ask.get().set_length(u32::MAX);
        let answer = ask.send().promise.await.unwrap();
        let first = answer.get().unwrap().get_data().unwrap().to_vec();
        assert_eq!(
            first.len(),
            MAX_PAGE_BYTES,
            "a read is cut to what a message carries"
        );
        assert_eq!(first[..], all[..MAX_PAGE_BYTES]);

        let rest = common::download(&c, file.id, u32::MAX).await.unwrap();
        assert_eq!(
            checksum(&rest),
            file.checksum,
            "reading on from where each answer ended"
        );
    })
    .await;
}
