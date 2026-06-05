use super::*;
use crate::storage::{DynStorage, InMemoryStorage, StoredBatchPayload};
use l2_core::{
    canonical_batch_data_bytes, canonical_batch_data_hash, crypto::sha256_bytes, Hash32, L2Block,
};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn da_roundtrip_writes_reads_and_verifies_payload() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = store(storage.clone(), 1024);
    let block = block(0);

    let written = da.write_batch_payload(&block).await.expect("write da");
    let loaded = da
        .read_batch_payload(block.header.height)
        .await
        .expect("read da")
        .expect("payload");
    let verified = da.verify_batch_payload(&block).await.expect("verify da");

    assert_eq!(written.block_height, block.header.height);
    assert_eq!(written.block_hash, block.header.block_hash());
    assert_eq!(written.data_hash, block.header.data_hash);
    assert_eq!(verified, written);
    assert_eq!(
        l2_core::crypto::hash_domain("l2.batch.data.v1", &[&loaded.payload_bytes]),
        block.header.data_hash
    );
    assert!(!storage
        .save_batch_payload(loaded)
        .await
        .expect("idempotent save"));
}

#[tokio::test]
async fn missing_payload_is_unavailable() {
    let da = store(Arc::new(InMemoryStorage::default()), 1024);
    let error = da
        .verify_batch_payload(&block(0))
        .await
        .expect_err("missing payload");

    assert!(matches!(error, DaError::Unavailable));
}

#[tokio::test]
async fn corrupted_partial_payload_is_rejected() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = store(storage.clone(), 1024);
    let block = block(0);
    storage
        .save_batch_payload(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: vec![0],
            public_ref: None,
            public_uri: None,
        })
        .await
        .expect("corrupt payload");

    let error = da
        .verify_batch_payload(&block)
        .await
        .expect_err("corrupt payload");

    assert!(matches!(error, DaError::HashMismatch { .. }));
}

#[tokio::test]
async fn old_payload_replayed_under_new_block_is_rejected() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = store(storage.clone(), 1024);
    let old = block(0);
    let new = block(1);
    let old_payload = canonical_batch_data_bytes(&old.transactions, &old.receipts);
    storage
        .save_batch_payload(StoredBatchPayload {
            block_height: new.header.height,
            block_hash: old.header.block_hash(),
            data_hash: new.header.data_hash,
            payload_bytes: old_payload,
            public_ref: None,
            public_uri: None,
        })
        .await
        .expect("replayed payload");

    let error = da
        .verify_batch_payload(&new)
        .await
        .expect_err("replayed payload");

    assert!(matches!(error, DaError::BlockHashMismatch { .. }));
}

#[tokio::test]
async fn oversized_payload_is_rejected_before_storage_write() {
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = store(storage.clone(), 1);
    let block = block(0);

    let error = da
        .write_batch_payload(&block)
        .await
        .expect_err("oversized payload");

    assert!(matches!(error, DaError::PayloadTooLarge { .. }));
    assert!(storage
        .get_batch_payload(block.header.height)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn filesystem_da_roundtrip_publishes_public_payload_and_mirror() {
    let root = temp_da_dir("roundtrip").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage.clone(), 1024, root.clone());
    let block = block(2);

    let written = da.write_batch_payload(&block).await.expect("write da");
    let stored = storage
        .get_batch_payload(block.header.height)
        .await
        .expect("storage read")
        .expect("stored payload");
    let public_ref = written.public_ref.as_deref().expect("public ref");
    let public_path = root.join(public_ref_path(public_ref));
    let public_bytes = tokio::fs::read(&public_path).await.expect("public bytes");
    let by_hash = da
        .read_batch_payload_by_hash(block.header.height, block.header.data_hash)
        .await
        .expect("read by hash")
        .expect("public payload");
    let verified = da.verify_batch_payload(&block).await.expect("verify");
    let expected_uri = format!("https://da.example.test/{public_ref}");

    assert_eq!(stored.public_ref.as_deref(), Some(public_ref));
    assert_eq!(stored.public_uri.as_deref(), Some(expected_uri.as_str()));
    assert_eq!(
        public_bytes,
        canonical_batch_data_bytes(&block.transactions, &block.receipts)
    );
    assert_eq!(by_hash.payload_bytes, public_bytes);
    assert_eq!(verified.public_ref.as_deref(), Some(public_ref));
}

#[tokio::test]
async fn filesystem_da_can_read_public_payload_without_postgres_mirror() {
    let root = temp_da_dir("fallback").await;
    let writer_storage: DynStorage = Arc::new(InMemoryStorage::default());
    let writer = filesystem_store(writer_storage, 1024, root.clone());
    let block = block(3);
    writer
        .write_batch_payload(&block)
        .await
        .expect("write public");

    let reader_storage: DynStorage = Arc::new(InMemoryStorage::default());
    let reader = filesystem_store(reader_storage, 1024, root);
    let loaded = reader
        .read_batch_payload_by_hash(block.header.height, block.header.data_hash)
        .await
        .expect("public read")
        .expect("payload");

    assert_eq!(loaded.block_hash, block.header.block_hash());
    assert_eq!(loaded.data_hash, block.header.data_hash);
    assert_eq!(
        loaded.payload_bytes,
        canonical_batch_data_bytes(&block.transactions, &block.receipts)
    );
}

#[tokio::test]
async fn filesystem_da_rejects_corrupted_public_payload() {
    let root = temp_da_dir("corrupt").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage, 1024, root.clone());
    let block = block(4);
    let written = da.write_batch_payload(&block).await.expect("write da");
    let public_path = root.join(public_ref_path(written.public_ref.as_deref().unwrap()));
    tokio::fs::write(public_path, [0u8]).await.expect("corrupt");

    let error = da
        .verify_batch_payload(&block)
        .await
        .expect_err("corrupt public payload");

    assert!(matches!(error, DaError::HashMismatch { .. }));
}

#[tokio::test]
async fn filesystem_da_requires_public_payload_before_relayer_commit() {
    let root = temp_da_dir("missing").await;
    let storage: DynStorage = Arc::new(InMemoryStorage::default());
    let da = filesystem_store(storage.clone(), 1024, root);
    let block = block(5);
    storage
        .save_batch_payload(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: canonical_batch_data_bytes(&block.transactions, &block.receipts),
            public_ref: None,
            public_uri: None,
        })
        .await
        .expect("storage mirror only");

    let error = da
        .verify_batch_payload(&block)
        .await
        .expect_err("missing public payload");

    assert!(matches!(error, DaError::Unavailable));
}

fn store(storage: DynStorage, max_payload_bytes: usize) -> StorageDaStore {
    StorageDaStore::new(
        storage,
        DataAvailabilityConfig {
            max_payload_bytes,
            public_backend: PublicDaBackend::PostgresOnly,
        },
    )
}

fn filesystem_store(
    storage: DynStorage,
    max_payload_bytes: usize,
    root: PathBuf,
) -> StorageDaStore {
    StorageDaStore::new(
        storage,
        DataAvailabilityConfig {
            max_payload_bytes,
            public_backend: PublicDaBackend::Filesystem {
                root_dir: root,
                base_url: Some("https://da.example.test".to_owned()),
            },
        },
    )
}

async fn temp_da_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("entropis-da-{name}-{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&path).await;
    path
}

fn public_ref_path(public_ref: &str) -> PathBuf {
    public_ref.split('/').collect::<PathBuf>()
}

fn block(height: u64) -> L2Block {
    L2Block::new(
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
