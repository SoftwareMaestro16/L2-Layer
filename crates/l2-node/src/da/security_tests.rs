use super::*;
use crate::storage::{DynStorage, InMemoryStorage, StoredBatchPayload};
use l2_core::{
    canonical_batch_data_bytes, canonical_batch_data_hash, crypto::sha256_bytes, Hash32,
};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn filesystem_da_rejects_stored_public_ref_path_traversal() {
    let root = temp_da_dir("path-traversal").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage.clone(), 1024, root, Some("https://da.example.test"));
    let block = empty_block(7);
    storage
        .save_batch_payload(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: canonical_batch_data_bytes(&block.transactions, &block.receipts),
            public_ref: Some("../escape.el2batch".to_owned()),
            public_uri: None,
        })
        .await
        .unwrap();

    let error = da
        .verify_batch_payload(&block)
        .await
        .expect_err("invalid public ref");

    assert!(matches!(error, DaError::InvalidPublicReference));
}

#[tokio::test]
async fn filesystem_da_rejects_stored_public_ref_backslash_traversal() {
    let root = temp_da_dir("backslash-traversal").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage.clone(), 1024, root, None);
    let block = empty_block(12);
    storage
        .save_batch_payload(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: canonical_batch_data_bytes(&block.transactions, &block.receipts),
            public_ref: Some("blocks\\..\\escape.el2batch".to_owned()),
            public_uri: None,
        })
        .await
        .unwrap();

    let error = da
        .verify_batch_payload(&block)
        .await
        .expect_err("invalid public ref");

    assert!(matches!(error, DaError::InvalidPublicReference));
}

#[tokio::test]
async fn filesystem_da_hash_lookup_rejects_ambiguous_public_payloads() {
    let root = temp_da_dir("ambiguous").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage, 1024, root.clone(), Some("https://da.example.test"));
    let block = empty_block(8);
    let payload = canonical_batch_data_bytes(&block.transactions, &block.receipts);
    let height_dir = root.join("blocks").join(block.header.height.to_string());
    tokio::fs::create_dir_all(&height_dir).await.unwrap();
    tokio::fs::write(
        height_dir.join(format!(
            "{}-{}.el2batch",
            hash(0x11).to_hex(),
            block.header.data_hash.to_hex()
        )),
        &payload,
    )
    .await
    .unwrap();
    tokio::fs::write(
        height_dir.join(format!(
            "{}-{}.el2batch",
            hash(0x12).to_hex(),
            block.header.data_hash.to_hex()
        )),
        &payload,
    )
    .await
    .unwrap();

    let error = da
        .read_batch_payload_by_hash(block.header.height, block.header.data_hash)
        .await
        .expect_err("ambiguous payload");

    assert!(matches!(error, DaError::AmbiguousPublicPayload));
}

#[tokio::test]
async fn filesystem_da_hash_lookup_rejects_invalid_public_filename() {
    let root = temp_da_dir("invalid-name").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage, 1024, root.clone(), None);
    let block = empty_block(9);
    let height_dir = root.join("blocks").join(block.header.height.to_string());
    tokio::fs::create_dir_all(&height_dir).await.unwrap();
    tokio::fs::write(
        height_dir.join(format!(
            "not-a-hash-{}.el2batch",
            block.header.data_hash.to_hex()
        )),
        canonical_batch_data_bytes(&block.transactions, &block.receipts),
    )
    .await
    .unwrap();

    let error = da
        .read_batch_payload_by_hash(block.header.height, block.header.data_hash)
        .await
        .expect_err("invalid public filename");

    assert!(matches!(error, DaError::InvalidPublicReference));
}

#[tokio::test]
async fn filesystem_da_hash_lookup_rejects_oversized_public_file_before_hashing() {
    let root = temp_da_dir("oversized-public").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage, 1, root.clone(), None);
    let block = empty_block(10);
    let height_dir = root.join("blocks").join(block.header.height.to_string());
    tokio::fs::create_dir_all(&height_dir).await.unwrap();
    tokio::fs::write(
        height_dir.join(format!(
            "{}-{}.el2batch",
            block.header.block_hash().to_hex(),
            block.header.data_hash.to_hex()
        )),
        [1u8, 2],
    )
    .await
    .unwrap();

    let error = da
        .read_batch_payload_by_hash(block.header.height, block.header.data_hash)
        .await
        .expect_err("oversized public payload");

    assert!(matches!(
        error,
        DaError::PayloadTooLarge { bytes: 2, max: 1 }
    ));
}

#[tokio::test]
async fn filesystem_da_without_base_url_publishes_public_ref_without_uri() {
    let root = temp_da_dir("no-base-url").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage.clone(), 1024, root, None);
    let block = empty_block(11);

    let written = da.write_batch_payload(&block).await.expect("write da");
    let stored = storage
        .get_batch_payload(block.header.height)
        .await
        .unwrap()
        .expect("stored payload");

    assert!(written.public_ref.is_some());
    assert!(written.public_uri.is_none());
    assert!(stored.public_ref.is_some());
    assert!(stored.public_uri.is_none());
}

fn filesystem_store(
    storage: DynStorage,
    max_payload_bytes: usize,
    root: PathBuf,
    base_url: Option<&str>,
) -> StorageDaStore {
    StorageDaStore::new(
        storage,
        DataAvailabilityConfig {
            max_payload_bytes,
            public_backend: PublicDaBackend::Filesystem {
                root_dir: root,
                base_url: base_url.map(str::to_owned),
            },
        },
    )
}

async fn temp_da_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "entropis-da-security-{name}-{}",
        std::process::id()
    ));
    let _ = tokio::fs::remove_dir_all(&path).await;
    path
}

fn empty_block(height: u64) -> l2_core::L2Block {
    l2_core::L2Block::new(
        height,
        sha256_bytes(b"prev-block"),
        Hash32::ZERO,
        sha256_bytes(&[height as u8]),
        vec![],
        vec![],
        vec![],
        canonical_batch_data_hash(&[], &[]),
        100 + height,
    )
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
