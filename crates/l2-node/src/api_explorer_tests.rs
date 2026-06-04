use super::*;
use axum::extract::Query;
use l2_core::crypto::sha256_bytes;
use l2_core::{
    canonical_batch_data_hash, L2TransactionKind, Receipt, SignedL2Transaction, WithdrawalLeaf,
};

const ADMIN_TOKEN: &str = "test-admin-token";

fn test_state(admin_token: Option<&str>) -> AppState {
    AppState::test(admin_token)
}

fn deposit_tx() -> SignedL2Transaction {
    SignedL2Transaction::system_deposit(
        "entropis-testnet",
        sha256_bytes(b"deposit"),
        1,
        sha256_bytes(b"recipient"),
        100,
    )
}

fn explorer_block(height: u64) -> L2Block {
    let tx = deposit_tx();
    let withdrawal = WithdrawalLeaf::new(
        sha256_bytes(b"withdrawal-tx"),
        1,
        50,
        sha256_bytes(b"sender"),
        "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR".to_owned(),
    );
    L2Block::new(
        height,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![tx.clone()],
        vec![Receipt::applied(tx.tx_hash(), 0, None)],
        vec![withdrawal],
        canonical_batch_data_hash(&[tx], &[]),
        100 + height,
    )
}

#[tokio::test]
async fn explorer_summary_and_blocks_are_public() {
    let state = test_state(Some(ADMIN_TOKEN));
    state.storage.save_block(explorer_block(0)).await.unwrap();

    let summary = explorer_summary(State(state.clone()))
        .await
        .expect("summary")
        .0;
    assert_eq!(summary.latest_block.as_ref().unwrap().height, 0);
    assert_eq!(summary.latest_block.as_ref().unwrap().deposit_count, 1);
    assert_eq!(summary.latest_block.as_ref().unwrap().withdrawal_count, 1);
    assert_eq!(
        summary.latest_batch_commit.as_ref().unwrap().status,
        "pending"
    );

    let blocks = explorer_blocks(
        State(state),
        Query(explorer::ExplorerListQuery {
            limit: Some(10),
            before_height: None,
        }),
    )
    .await
    .expect("blocks")
    .0;
    assert_eq!(blocks.items.len(), 1);
    assert_eq!(blocks.items[0].height, 0);
}

#[tokio::test]
async fn explorer_lists_and_finds_included_deposit() {
    let state = test_state(Some(ADMIN_TOKEN));
    let block = explorer_block(0);
    let L2TransactionKind::Deposit { deposit_id, .. } = block.transactions[0].kind else {
        panic!("deposit tx")
    };
    state.storage.save_block(block).await.unwrap();

    let deposits = explorer_deposits(
        State(state.clone()),
        Query(explorer::ExplorerListQuery {
            limit: Some(10),
            before_height: None,
        }),
    )
    .await
    .expect("deposits")
    .0;
    assert_eq!(deposits.items.len(), 1);
    assert_eq!(deposits.items[0].deposit.deposit_id, deposit_id);
    assert_eq!(deposits.items[0].status, "included");

    let found = explorer_deposit(State(state), Path(deposit_id.to_hex()))
        .await
        .expect("deposit")
        .0;
    assert_eq!(found.deposit.deposit_id, deposit_id);
}

#[tokio::test]
async fn explorer_withdrawal_status_tracks_finalization_without_claim_proof() {
    let state = test_state(Some(ADMIN_TOKEN));
    let block = explorer_block(0);
    let withdrawal_id = block.withdrawals[0].withdrawal_id;
    state.storage.save_block(block).await.unwrap();

    let status = explorer_withdrawal(State(state.clone()), Path(withdrawal_id.to_hex()))
        .await
        .expect("withdrawal status")
        .0;
    assert_eq!(status.status, "waiting_for_finalization");
    assert!(!status.proof_available);

    let commit = state.storage.get_batch_commit(1).await.unwrap().unwrap();
    let mut finalization = crate::storage::BatchFinalizationRecord::pending(&commit, 0);
    finalization.status = crate::storage::BatchFinalizationStatus::Finalized;
    state
        .storage
        .save_batch_finalization(finalization)
        .await
        .unwrap();

    let finalized = explorer_withdrawal(State(state), Path(withdrawal_id.to_hex()))
        .await
        .expect("finalized withdrawal status")
        .0;
    assert_eq!(finalized.status, "finalized");
    assert!(finalized.proof_available);
}
