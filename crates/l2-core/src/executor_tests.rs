use super::*;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::State;
use crate::types::{L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET};
use crate::{sample_counter_initial_state, GasSchedule};

const CHAIN_ID: &str = "entropis-testnet";

fn account(seed: &[u8]) -> Hash32 {
    sha256_bytes(seed)
}

fn tx(
    from: Hash32,
    nonce: u64,
    gas_limit: u64,
    max_gas_price: u128,
    kind: L2TransactionKind,
) -> SignedL2Transaction {
    SignedL2Transaction {
        chain_id: CHAIN_ID.to_owned(),
        from: Some(from),
        nonce,
        gas_limit,
        max_gas_price,
        kind,
        public_key: None,
        signature: None,
    }
}

fn config(gas_schedule: GasSchedule) -> ExecutionConfig {
    ExecutionConfig {
        block_height: 7,
        gas_schedule,
        ..ExecutionConfig::default()
    }
}

#[test]
fn deploy_contract_sets_hashes_and_charges_call_gas() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    let sample = sample_counter_initial_state(0);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().call_contract_gas,
            2,
            L2TransactionKind::DeployContract {
                contract,
                code_hash: sample.code_hash,
                data_hash: sample.data_hash,
                storage_root: sample.storage_root,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 100);
    assert_eq!(state.account(sender).unwrap().balance(0), 0);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert_eq!(state.account(contract).unwrap().code_hash, sample.code_hash);
    assert_eq!(state.account(contract).unwrap().data_hash, sample.data_hash);
    assert_eq!(
        state.account(contract).unwrap().storage_root,
        sample.storage_root
    );
}

#[test]
fn deploy_contract_rejects_overwrite_without_corrupting_existing_contract() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    let initial = sample_counter_initial_state(3);
    let replacement = sample_counter_initial_state(9);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));
    state.account_mut(contract).code_hash = initial.code_hash;
    state.account_mut(contract).data_hash = initial.data_hash;
    state.account_mut(contract).storage_root = initial.storage_root;

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().call_contract_gas,
            1,
            L2TransactionKind::DeployContract {
                contract,
                code_hash: replacement.code_hash,
                data_hash: replacement.data_hash,
                storage_root: replacement.storage_root,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("contract_already_exists")
    );
    assert_eq!(
        state.account(contract).unwrap().storage_root,
        initial.storage_root
    );
}

#[test]
fn transfer_uses_configured_gas_schedule_and_price() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));

    let schedule = GasSchedule {
        transfer_gas: 7,
        ..GasSchedule::default()
    };
    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            7,
            4,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 30,
            },
        ),
        &config(schedule),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 28);
    assert_eq!(
        state.account(sender).unwrap().balance(L2_NATIVE_GAS_ASSET),
        42
    );
    assert_eq!(
        state
            .account(recipient)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        30
    );
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn exact_gas_limit_is_accepted() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 50));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().transfer_gas,
            1,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 40,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 10);
    assert_eq!(state.account(sender).unwrap().balance(0), 0);
    assert_eq!(state.account(recipient).unwrap().balance(0), 40);
}

#[test]
fn insufficient_gas_limit_charges_rejection_fee_and_advances_nonce() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            9,
            3,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 10,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("insufficient_gas_limit")
    );
    assert_eq!(outcome.receipt.gas_charged, 3);
    assert_eq!(state.account(sender).unwrap().balance(0), 97);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert!(state.account(recipient).is_none());
}

#[test]
fn insufficient_gas_coin_rejects_even_when_transferred_asset_is_sufficient() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(1, 100));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            1,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: 1,
                amount: 50,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("insufficient_gas_coin")
    );
    assert_eq!(outcome.receipt.gas_charged, 0);
    assert_eq!(state.account(sender).unwrap().balance(1), 100);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert!(state.account(recipient).is_none());
}

#[test]
fn multi_asset_transfer_debits_asset_and_gas_separately() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));
    assert!(state.account_mut(sender).credit(2, 90));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            2,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: 2,
                amount: 40,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 20);
    assert_eq!(state.account(sender).unwrap().balance(0), 80);
    assert_eq!(state.account(sender).unwrap().balance(2), 50);
    assert_eq!(state.account(recipient).unwrap().balance(2), 40);
}

#[test]
fn amount_plus_gas_overflow_rejects_without_transfer() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state
        .account_mut(sender)
        .credit(L2_NATIVE_GAS_ASSET, u128::MAX));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            1,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: u128::MAX,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("fee_overflow"));
    assert_eq!(outcome.receipt.gas_charged, 1);
    assert_eq!(
        state.account(sender).unwrap().balance(L2_NATIVE_GAS_ASSET),
        u128::MAX - 1
    );
    assert!(state.account(recipient).is_none());
}
