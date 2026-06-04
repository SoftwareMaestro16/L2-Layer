use super::*;
use l2_core::{crypto::sha256_bytes, L2Block};

fn deposit_event() -> DepositEvent {
    DepositEvent {
        deposit_id: sha256_bytes(b"deposit"),
        asset_id: 0,
        recipient: sha256_bytes(b"recipient"),
        amount: 100,
        l1_tx_hash: sha256_bytes(b"l1-tx"),
        l1_lt: 1,
    }
}

#[tokio::test]
async fn memory_storage_deposit_idempotency_rejects_replay() {
    let storage = InMemoryStorage::default();
    let deposit = deposit_event();

    assert!(storage.save_deposit(deposit.clone()).await.unwrap());
    assert!(!storage.save_deposit(deposit).await.unwrap());
}

#[tokio::test]
async fn memory_storage_deposit_l1_cursor_rejects_replay() {
    let storage = InMemoryStorage::default();
    let deposit = deposit_event();
    let mut replay = deposit.clone();
    replay.deposit_id = sha256_bytes(b"different-deposit-id");

    assert!(storage.save_deposit(deposit).await.unwrap());
    assert!(!storage.save_deposit(replay).await.unwrap());
}

#[tokio::test]
async fn memory_storage_ent_faucet_grant_is_one_per_account() {
    let storage = InMemoryStorage::default();
    let account = sha256_bytes(b"account");

    assert!(storage.save_ent_faucet_grant(account, 1_000).await.unwrap());
    assert!(!storage.save_ent_faucet_grant(account, 1_000).await.unwrap());
}

#[tokio::test]
async fn memory_storage_batch_payload_is_idempotent_and_rejects_conflict() {
    let storage = InMemoryStorage::default();
    let payload = StoredBatchPayload {
        block_height: 1,
        block_hash: sha256_bytes(b"block"),
        data_hash: sha256_bytes(b"data"),
        payload_bytes: vec![1, 2, 3],
    };

    assert!(storage.save_batch_payload(payload.clone()).await.unwrap());
    assert!(!storage.save_batch_payload(payload.clone()).await.unwrap());

    let mut conflicting = payload.clone();
    conflicting.payload_bytes = vec![9];
    let error = storage
        .save_batch_payload(conflicting)
        .await
        .expect_err("conflict");
    assert!(matches!(
        error,
        StorageError::Conflict {
            resource: "batch payload"
        }
    ));
    assert_eq!(storage.get_batch_payload(1).await.unwrap(), Some(payload));
}

#[tokio::test]
async fn memory_storage_block_lookup_is_reproducible() {
    let storage = InMemoryStorage::default();
    let block = L2Block::new(
        7,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        sha256_bytes(b"data"),
        100,
    );
    storage.save_block(block.clone()).await.unwrap();

    let loaded = storage.get_block(7).await.unwrap().expect("block");
    assert_eq!(loaded.header.block_hash(), block.header.block_hash());
    assert!(storage.get_block(8).await.unwrap().is_none());
}

#[tokio::test]
async fn memory_storage_creates_pending_batch_commit_for_block() {
    let storage = InMemoryStorage::default();
    let block = L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        sha256_bytes(b"data"),
        100,
    );
    storage.save_block(block.clone()).await.unwrap();

    let record = storage
        .get_batch_commit(1)
        .await
        .unwrap()
        .expect("commit record");
    assert_eq!(record.batch_no, 1);
    assert_eq!(record.block_height, 0);
    assert_eq!(record.block_hash, block.header.block_hash());
    assert_eq!(record.status, BatchCommitStatus::Pending);
}

#[tokio::test]
async fn memory_storage_lists_and_updates_batch_commit_status() {
    let storage = InMemoryStorage::default();
    let block = L2Block::new(
        2,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        sha256_bytes(b"data"),
        100,
    );
    storage.save_block(block).await.unwrap();
    let mut record = storage.get_batch_commit(3).await.unwrap().unwrap();
    record.status = BatchCommitStatus::Failed;
    record.attempts = 1;
    record.last_error = Some("network".to_owned());
    storage.save_batch_commit(record.clone()).await.unwrap();

    let failed = storage
        .list_batch_commits(&[BatchCommitStatus::Failed], 2, 10)
        .await
        .unwrap();
    assert_eq!(failed, vec![record]);
    assert!(storage
        .list_batch_commits(&[BatchCommitStatus::Failed], 1, 10)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn memory_storage_cursor_roundtrip() {
    let storage = InMemoryStorage::default();
    let cursor = L1Cursor {
        lt: 42,
        hash: sha256_bytes(b"cursor"),
    };

    storage
        .set_l1_cursor("vault", cursor.clone())
        .await
        .unwrap();
    assert_eq!(storage.get_l1_cursor("vault").await.unwrap(), Some(cursor));
    assert!(storage.get_l1_cursor("missing").await.unwrap().is_none());
}
