use super::*;
use crate::storage::{DynStorage, InMemoryStorage, StoredBatchPayload};
use l2_core::{
    canonical_batch_data_bytes, canonical_batch_data_hash, crypto::sha256_bytes, Hash32, L2Block,
};
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

fn store(storage: DynStorage, max_payload_bytes: usize) -> StorageDaStore {
    StorageDaStore::new(storage, DataAvailabilityConfig { max_payload_bytes })
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
