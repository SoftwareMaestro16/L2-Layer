use super::*;
use axum::extract::Query;
use l2_core::crypto::{derive_account_id, sha256_bytes};
use l2_core::{
    canonical_batch_data_hash, l2_raw_address, l2_user_friendly_address, L2TransactionKind,
    Receipt, SignedL2Transaction, WithdrawalLeaf, ENWALLET_V5R1_CODE_HASH, ENWALLET_V5R1_INTERFACE,
    ENWALLET_V5R1_LABEL, L2_ZERO_ADDRESS_INTERFACE, L2_ZERO_ADDRESS_LABEL,
    L2_ZERO_FRIENDLY_ADDRESS, L2_ZERO_RAW_ADDRESS,
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

fn user_tx(from: Hash32, nonce: u64, kind: L2TransactionKind) -> SignedL2Transaction {
    SignedL2Transaction {
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        gas_limit: 1_000,
        max_gas_price: 2,
        kind,
        public_key: Some(hex::encode([7u8; 32])),
        signature: Some(hex::encode([8u8; 64])),
    }
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
async fn explorer_account_returns_addresses_and_balances() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"account");
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(account_id);
        account.nonce = 2;
        account.credit(0, 123);
        account.last_lt = 9;
    }

    let account = explorer_account(State(state), Path(l2_raw_address(account_id)))
        .await
        .expect("account")
        .0;

    assert_eq!(account.account_id, account_id);
    assert_eq!(account.raw_address, l2_raw_address(account_id));
    assert_eq!(
        account.user_friendly_address,
        l2_user_friendly_address(account_id)
    );
    assert_eq!(account.status, "active");
    assert_eq!(account.nonce, 2);
    assert_eq!(account.last_lt, 9);
    assert_eq!(account.balances[0].asset_id, 0);
    assert_eq!(account.balances[0].amount, 123);
    assert!(account.interfaces.is_empty());
}

#[tokio::test]
async fn explorer_account_marks_enwallet_v5_interface() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"enwallet");
    {
        let mut sequencer = state.sequencer.write().await;
        sequencer.state.account_mut(account_id).code_hash = ENWALLET_V5R1_CODE_HASH;
    }

    let account = explorer_account(State(state), Path(l2_raw_address(account_id)))
        .await
        .expect("account")
        .0;

    assert_eq!(account.interfaces.len(), 1);
    assert_eq!(account.interfaces[0].id, ENWALLET_V5R1_INTERFACE);
    assert_eq!(account.interfaces[0].label, ENWALLET_V5R1_LABEL);
}

#[tokio::test]
async fn explorer_account_marks_zero_address_reserved() {
    let state = test_state(Some(ADMIN_TOKEN));

    let account = explorer_account(State(state), Path(L2_ZERO_FRIENDLY_ADDRESS.to_owned()))
        .await
        .expect("zero account")
        .0;

    assert_eq!(account.account_id, Hash32::ZERO);
    assert_eq!(account.raw_address, L2_ZERO_RAW_ADDRESS);
    assert_eq!(account.user_friendly_address, L2_ZERO_FRIENDLY_ADDRESS);
    assert_eq!(account.status, "reserved");
    assert_eq!(account.balances.len(), 0);
    assert_eq!(account.interfaces.len(), 1);
    assert_eq!(account.interfaces[0].id, L2_ZERO_ADDRESS_INTERFACE);
    assert_eq!(account.interfaces[0].label, L2_ZERO_ADDRESS_LABEL);
}

#[tokio::test]
async fn explorer_account_rejects_invalid_address() {
    let state = test_state(Some(ADMIN_TOKEN));
    let error = explorer_account(State(state), Path("not-an-address".to_owned()))
        .await
        .expect_err("invalid account");

    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn explorer_account_transactions_are_paginated_newest_first() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = derive_account_id(&[1u8; 32]);
    let recipient = sha256_bytes(b"recipient");
    let first = user_tx(
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 10,
        },
    );
    let second = user_tx(
        recipient,
        0,
        L2TransactionKind::Transfer {
            to: account_id,
            asset_id: 0,
            amount: 5,
        },
    );
    let contract = user_tx(
        recipient,
        1,
        L2TransactionKind::CallContract {
            contract: account_id,
            body_boc_base64: "te6ccgEBAQEAAgAAAA==".to_owned(),
        },
    );
    let block0 = L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state-0"),
        vec![first.clone()],
        vec![Receipt::applied(first.tx_hash(), 4, None)],
        vec![],
        canonical_batch_data_hash(&[first.clone()], &[]),
        100,
    );
    let block1 = L2Block::new(
        1,
        block0.header.block_hash(),
        block0.header.state_root,
        sha256_bytes(b"state-1"),
        vec![second.clone(), contract.clone()],
        vec![
            Receipt::applied(second.tx_hash(), 6, None),
            Receipt::rejected_with_gas(contract.tx_hash(), "contract_error", 8),
        ],
        vec![],
        canonical_batch_data_hash(&[second.clone(), contract.clone()], &[]),
        101,
    );
    state.storage.save_block(block0).await.unwrap();
    state.storage.save_block(block1).await.unwrap();

    let first_page = explorer_account_transactions(
        State(state.clone()),
        Path(account_id.to_hex()),
        Query(explorer::account::ExplorerAccountTransactionsQuery {
            limit: Some(2),
            before_height: None,
            before_index: None,
        }),
    )
    .await
    .expect("first page")
    .0;

    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[0].kind, "call_contract");
    assert_eq!(first_page.items[0].direction, "in");
    assert_eq!(first_page.items[0].status, "rejected");
    assert_eq!(first_page.items[0].gas_charged.as_deref(), Some("8"));
    assert_eq!(
        first_page.items[0].reason.as_deref(),
        Some("contract_error")
    );
    assert_eq!(first_page.items[1].kind, "transfer");
    assert_eq!(first_page.items[1].direction, "in");
    let cursor = first_page.next_cursor.expect("cursor");

    let second_page = explorer_account_transactions(
        State(state),
        Path(account_id.to_hex()),
        Query(explorer::account::ExplorerAccountTransactionsQuery {
            limit: Some(2),
            before_height: Some(cursor.before_height),
            before_index: Some(cursor.before_index),
        }),
    )
    .await
    .expect("second page")
    .0;

    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].tx_hash, first.tx_hash());
    assert_eq!(second_page.items[0].direction, "out");
    assert!(second_page.next_cursor.is_none());
}

#[tokio::test]
async fn explorer_tx_returns_detail_roots_and_raw_payload() {
    let state = test_state(Some(ADMIN_TOKEN));
    let from = sha256_bytes(b"sender");
    let to = sha256_bytes(b"recipient");
    let tx = user_tx(
        from,
        7,
        L2TransactionKind::Transfer {
            to,
            asset_id: 1,
            amount: 55,
        },
    );
    let receipt = Receipt::applied(tx.tx_hash(), 12, None);
    let block = L2Block::new(
        3,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![tx.clone()],
        vec![receipt],
        vec![],
        canonical_batch_data_hash(std::slice::from_ref(&tx), &[]),
        222,
    );
    let tx_root = block.header.tx_root;
    state.storage.save_block(block).await.unwrap();

    let detail = explorer_tx(State(state), Path(tx.tx_hash().to_hex()))
        .await
        .expect("tx detail")
        .0;

    assert_eq!(detail.summary.tx_hash, tx.tx_hash());
    assert_eq!(detail.summary.block_height, 3);
    assert_eq!(detail.summary.timestamp, 222);
    assert_eq!(detail.tx_root, tx_root);
    assert_eq!(detail.summary.kind, "transfer");
    assert_eq!(detail.summary.asset_id, Some(1));
    assert_eq!(detail.summary.amount.as_deref(), Some("55"));
    assert_eq!(detail.summary.status, "applied");
    assert_eq!(detail.summary.gas_charged.as_deref(), Some("12"));
    assert_eq!(detail.chain_id, "entropis-testnet");
    assert_eq!(detail.nonce, 7);
    assert_eq!(detail.gas_limit, 1_000);
    assert_eq!(detail.max_gas_price, 2);
    assert_eq!(detail.raw_transaction.tx_hash(), tx.tx_hash());
}

#[tokio::test]
async fn explorer_tx_rejects_invalid_hash() {
    let state = test_state(Some(ADMIN_TOKEN));
    let error = explorer_tx(State(state), Path("not-a-hash".to_owned()))
        .await
        .expect_err("invalid tx");

    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
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
