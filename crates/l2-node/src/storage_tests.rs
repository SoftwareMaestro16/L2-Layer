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
        public_ref: None,
        public_uri: None,
    };

    assert!(storage.save_batch_payload(payload.clone()).await.unwrap());
    assert!(!storage.save_batch_payload(payload.clone()).await.unwrap());

    let mut public_payload = payload.clone();
    public_payload.public_ref = Some("blocks/1/block-data.el2batch".to_owned());
    public_payload.public_uri =
        Some("https://da.example.test/blocks/1/block-data.el2batch".to_owned());
    assert!(!storage
        .save_batch_payload(public_payload.clone())
        .await
        .unwrap());
    assert_eq!(
        storage.get_batch_payload(1).await.unwrap(),
        Some(public_payload.clone())
    );

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
    assert_eq!(
        storage.get_batch_payload(1).await.unwrap(),
        Some(public_payload)
    );
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
async fn memory_storage_reads_latest_batch_commit_by_status() {
    let storage = InMemoryStorage::default();
    storage
        .save_block(L2Block::new(
            0,
            Hash32::ZERO,
            Hash32::ZERO,
            sha256_bytes(b"state-0"),
            vec![],
            vec![],
            vec![],
            sha256_bytes(b"data-0"),
            100,
        ))
        .await
        .unwrap();
    storage
        .save_block(L2Block::new(
            1,
            Hash32::ZERO,
            sha256_bytes(b"state-0"),
            sha256_bytes(b"state-1"),
            vec![],
            vec![],
            vec![],
            sha256_bytes(b"data-1"),
            101,
        ))
        .await
        .unwrap();

    let mut second = storage.get_batch_commit(2).await.unwrap().unwrap();
    second.status = BatchCommitStatus::Confirmed;
    storage.save_batch_commit(second.clone()).await.unwrap();

    assert_eq!(
        storage.latest_batch_commit(&[]).await.unwrap().unwrap(),
        second
    );
    assert_eq!(
        storage
            .latest_batch_commit(&[BatchCommitStatus::Confirmed])
            .await
            .unwrap()
            .unwrap(),
        second
    );
    assert!(storage
        .latest_batch_commit(&[BatchCommitStatus::Failed])
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn memory_storage_lists_and_reads_batch_finalizations() {
    let storage = InMemoryStorage::default();
    storage
        .save_block(L2Block::new(
            0,
            Hash32::ZERO,
            Hash32::ZERO,
            sha256_bytes(b"state"),
            vec![],
            vec![],
            vec![],
            sha256_bytes(b"data"),
            100,
        ))
        .await
        .unwrap();
    let commit = storage.get_batch_commit(1).await.unwrap().unwrap();
    let mut record = BatchFinalizationRecord::pending(&commit, 123);
    storage
        .save_batch_finalization(record.clone())
        .await
        .unwrap();

    assert_eq!(
        storage.get_batch_finalization(1).await.unwrap(),
        Some(record.clone())
    );
    assert_eq!(
        storage
            .list_batch_finalizations(&[BatchFinalizationStatus::Pending], 1, 10)
            .await
            .unwrap(),
        vec![record.clone()]
    );

    record.status = BatchFinalizationStatus::Finalized;
    record.message_hash_norm = Some(sha256_bytes(b"finalize"));
    storage
        .save_batch_finalization(record.clone())
        .await
        .unwrap();

    assert_eq!(
        storage
            .latest_batch_finalization(&[BatchFinalizationStatus::Finalized])
            .await
            .unwrap(),
        Some(record)
    );
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
