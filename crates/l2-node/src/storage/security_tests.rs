use super::*;
use l2_core::{
    crypto::sha256_bytes, sample_counter_initial_state, Account, InternalMessageQueue, L2Block,
    Receipt, SignedL2Transaction, TvmInternalMessage,
};

#[tokio::test]
async fn memory_storage_block_overwrite_removes_stale_transaction_index() {
    let storage = InMemoryStorage::default();
    let first_tx = deposit_tx(b"old-deposit", b"old-recipient");
    let first_hash = first_tx.tx_hash();
    let second_tx = deposit_tx(b"new-deposit", b"new-recipient");
    let second_hash = second_tx.tx_hash();

    storage
        .save_block(block_with_transactions(0, b"first-state", vec![first_tx]))
        .await
        .unwrap();
    assert!(storage.get_transaction(first_hash).await.unwrap().is_some());

    storage
        .save_block(block_with_transactions(0, b"second-state", vec![second_tx]))
        .await
        .unwrap();

    assert!(storage.get_transaction(first_hash).await.unwrap().is_none());
    let stored = storage
        .get_transaction(second_hash)
        .await
        .unwrap()
        .expect("new transaction");
    assert_eq!(stored.block_height, 0);
    assert_eq!(stored.tx_index, 0);
    let stats = storage.explorer_storage_stats().await.unwrap();
    assert_eq!(stats.block_count, 1);
    assert_eq!(stats.transaction_count, 1);
    assert_eq!(stats.deposit_count, 1);
}

#[tokio::test]
async fn memory_storage_cursor_rejects_rollback_and_same_lt_hash_change() {
    let storage = InMemoryStorage::default();
    let initial = L1Cursor {
        lt: 10,
        hash: hash(b"cursor-10"),
    };

    storage
        .set_l1_cursor("vault", initial.clone())
        .await
        .unwrap();
    storage
        .set_l1_cursor("vault", initial.clone())
        .await
        .unwrap();

    let rollback = storage
        .set_l1_cursor(
            "vault",
            L1Cursor {
                lt: 9,
                hash: hash(b"cursor-9"),
            },
        )
        .await
        .expect_err("cursor rollback");
    assert!(matches!(
        rollback,
        StorageError::Conflict {
            resource: "l1 cursor"
        }
    ));

    let fork = storage
        .set_l1_cursor(
            "vault",
            L1Cursor {
                lt: 10,
                hash: hash(b"cursor-10-fork"),
            },
        )
        .await
        .expect_err("same lt different hash");
    assert!(matches!(
        fork,
        StorageError::Conflict {
            resource: "l1 cursor"
        }
    ));
    assert_eq!(storage.get_l1_cursor("vault").await.unwrap(), Some(initial));

    let advanced = L1Cursor {
        lt: 11,
        hash: hash(b"cursor-11"),
    };
    storage
        .set_l1_cursor("vault", advanced.clone())
        .await
        .unwrap();
    assert_eq!(
        storage.get_l1_cursor("vault").await.unwrap(),
        Some(advanced)
    );
}

#[tokio::test]
async fn memory_storage_rejects_invalid_observer_checkpoint_without_replacing_valid() {
    let storage = InMemoryStorage::default();
    let valid = checkpoint(4);
    storage
        .save_observer_checkpoint(valid.clone())
        .await
        .unwrap();

    let mut poisoned = valid.clone();
    poisoned.state_root = hash(b"poisoned-state-root");
    let error = storage
        .save_observer_checkpoint(poisoned)
        .await
        .expect_err("invalid checkpoint");

    assert!(matches!(
        error,
        StorageError::InvalidObserverCheckpoint {
            reason: "state root mismatch"
        }
    ));
    assert_eq!(
        storage.latest_observer_checkpoint().await.unwrap(),
        Some(valid)
    );
}

#[tokio::test]
async fn memory_storage_lower_observer_checkpoint_does_not_roll_back_latest() {
    let storage = InMemoryStorage::default();
    let later = checkpoint(5);
    let earlier = checkpoint(4);

    storage
        .save_observer_checkpoint(later.clone())
        .await
        .unwrap();
    storage.save_observer_checkpoint(earlier).await.unwrap();

    assert_eq!(
        storage.latest_observer_checkpoint().await.unwrap(),
        Some(later)
    );
}

#[tokio::test]
async fn memory_storage_contract_cell_conflict_preserves_existing_state() {
    let storage = InMemoryStorage::default();
    let contract = hash(b"contract");
    let record = contract_record(contract, 3, 12);
    storage.save_contract_state(record.clone()).await.unwrap();

    let mut poisoned = record.clone();
    poisoned.code_cell.code_boc_base64 = "conflicting-code-boc".to_owned();
    let error = storage
        .save_contract_state(poisoned)
        .await
        .expect_err("code cell conflict");

    assert!(matches!(
        error,
        StorageError::Conflict {
            resource: "contract code cell"
        }
    ));
    assert_eq!(
        storage.get_contract_state(contract).await.unwrap(),
        Some(record)
    );
}

#[tokio::test]
async fn memory_storage_rejects_stale_contract_state_height_update() {
    let storage = InMemoryStorage::default();
    let contract = hash(b"stale-contract");
    let latest = contract_record(contract, 7, 12);
    let stale = contract_record(contract, 8, 11);

    storage.save_contract_state(latest.clone()).await.unwrap();
    storage.save_contract_state(stale).await.unwrap();

    assert_eq!(
        storage.get_contract_state(contract).await.unwrap(),
        Some(latest)
    );
}

#[tokio::test]
async fn memory_storage_batch_payload_public_ref_conflict_preserves_existing() {
    let storage = InMemoryStorage::default();
    let payload = StoredBatchPayload {
        block_height: 3,
        block_hash: hash(b"block"),
        data_hash: hash(b"data"),
        payload_bytes: vec![1, 2, 3],
        public_ref: Some("blocks/3/a.el2batch".to_owned()),
        public_uri: Some("https://da.example.test/blocks/3/a.el2batch".to_owned()),
    };
    storage.save_batch_payload(payload.clone()).await.unwrap();

    let mut conflicting = payload.clone();
    conflicting.public_ref = Some("blocks/3/b.el2batch".to_owned());
    conflicting.public_uri = Some("https://da.example.test/blocks/3/b.el2batch".to_owned());
    let error = storage
        .save_batch_payload(conflicting)
        .await
        .expect_err("public ref conflict");

    assert!(matches!(
        error,
        StorageError::Conflict {
            resource: "batch payload"
        }
    ));
    assert_eq!(storage.get_batch_payload(3).await.unwrap(), Some(payload));
}

#[tokio::test]
async fn memory_storage_internal_queue_snapshot_restores_fifo_order() {
    let storage = InMemoryStorage::default();
    let mut queue = InternalMessageQueue::new(4);
    queue
        .push_many(7, vec![internal_message(b"a"), internal_message(b"b")])
        .expect("queue messages");
    storage
        .save_internal_queue_snapshot(InternalQueueSnapshotRecord {
            block_height: 7,
            queue: queue.snapshot(),
        })
        .await
        .unwrap();

    let snapshot = storage
        .latest_internal_queue_snapshot()
        .await
        .unwrap()
        .expect("snapshot");
    let mut restored =
        InternalMessageQueue::from_snapshot(4, snapshot.queue).expect("restore queue");
    let first = restored.pop_front().expect("first");
    let second = restored.pop_front().expect("second");

    assert_eq!(first.sequence, 0);
    assert_eq!(second.sequence, 1);
    assert_eq!(first.message.body_boc, b"a");
    assert_eq!(second.message.body_boc, b"b");
    assert!(restored.is_empty());
}

fn block_with_transactions(
    height: u64,
    state_seed: &[u8],
    transactions: Vec<SignedL2Transaction>,
) -> L2Block {
    let receipts = transactions
        .iter()
        .map(|tx| Receipt::applied(tx.tx_hash(), 0, None))
        .collect::<Vec<_>>();
    L2Block::new(
        height,
        Hash32::ZERO,
        Hash32::ZERO,
        hash(state_seed),
        transactions,
        receipts,
        vec![],
        hash(b"data"),
        100 + height,
    )
}

fn deposit_tx(deposit_seed: &[u8], recipient_seed: &[u8]) -> SignedL2Transaction {
    SignedL2Transaction::system_deposit(
        "entropis-testnet",
        hash(deposit_seed),
        1,
        hash(recipient_seed),
        100,
    )
}

fn checkpoint(next_batch_no: u64) -> ObserverCheckpoint {
    let mut checkpoint = ObserverCheckpoint::genesis();
    checkpoint.next_batch_no = next_batch_no;
    checkpoint.next_block_height = next_batch_no.saturating_sub(1);
    checkpoint
}

fn contract_record(account_id: Hash32, counter: u64, block_height: u64) -> StoredContractState {
    let sample = sample_counter_initial_state(counter);
    let mut account = Account::default();
    account.mark_contract_account();
    account.code_hash = sample.code_hash;
    account.data_hash = sample.data_hash;
    account.storage_root = sample.storage_root;
    account.code_boc_base64 = Some(sample.code_boc_base64);
    account.data_boc_base64 = Some(sample.data_boc_base64);
    StoredContractState::from_account(account_id, &account, block_height)
        .unwrap()
        .expect("contract state")
}

fn internal_message(body: &[u8]) -> TvmInternalMessage {
    TvmInternalMessage {
        from: hash(b"from"),
        to: hash(b"to"),
        value: 0,
        body_boc: body.to_vec(),
        bounce: true,
        bounced: false,
    }
}

fn hash(seed: &[u8]) -> Hash32 {
    sha256_bytes(seed)
}
