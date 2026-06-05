use super::*;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::{AccountFlags, AccountType, State};
use crate::types::{
    L2Event, L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use crate::{sample_counter_initial_state, FeeAccountingConfig, GasSchedule};

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
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: CHAIN_ID.to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit,
        max_gas_price,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
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
                code_boc_base64: sample.code_boc_base64.clone(),
                data_boc_base64: sample.data_boc_base64.clone(),
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
    assert_eq!(
        state.account(contract).unwrap().account_type,
        AccountType::Contract
    );
    assert!(state.account(contract).unwrap().flags.contract_only);
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
    state.account_mut(contract).code_boc_base64 = Some(initial.code_boc_base64);
    state.account_mut(contract).data_boc_base64 = Some(initial.data_boc_base64);

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().call_contract_gas,
            1,
            L2TransactionKind::DeployContract {
                contract,
                code_boc_base64: replacement.code_boc_base64,
                data_boc_base64: replacement.data_boc_base64,
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
fn deploy_contract_initializes_prefunded_uninitialized_account() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"prefunded-wallet");
    let sample = sample_counter_initial_state(0);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));
    assert!(state.account_mut(contract).credit(L2_NATIVE_GAS_ASSET, 250));
    state.account_mut(contract).last_lt = 3;

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().call_contract_gas,
            1,
            L2TransactionKind::DeployContract {
                contract,
                code_boc_base64: sample.code_boc_base64.clone(),
                data_boc_base64: sample.data_boc_base64.clone(),
            },
        ),
        &ExecutionConfig::default(),
    );

    let deployed = state.account(contract).unwrap();
    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(deployed.balance(L2_NATIVE_GAS_ASSET), 250);
    assert_eq!(deployed.code_hash, sample.code_hash);
    assert_eq!(deployed.data_hash, sample.data_hash);
    assert_eq!(deployed.storage_root, sample.storage_root);
    assert_eq!(deployed.account_type, AccountType::Contract);
    assert!(deployed.flags.contract_only);
}

#[test]
fn deploy_contract_rejects_claimed_user_account() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let claimed_user = account(b"claimed-user");
    let sample = sample_counter_initial_state(0);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));
    let account = state.account_mut(claimed_user);
    account.active_public_key = Some(Hash32::new([7; 32]));
    account.credit(L2_NATIVE_GAS_ASSET, 25);

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            GasSchedule::default().call_contract_gas,
            1,
            L2TransactionKind::DeployContract {
                contract: claimed_user,
                code_boc_base64: sample.code_boc_base64,
                data_boc_base64: sample.data_boc_base64,
            },
        ),
        &ExecutionConfig::default(),
    );

    let account = state.account(claimed_user).unwrap();
    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("contract_already_exists")
    );
    assert_eq!(account.account_type, AccountType::User);
    assert_eq!(account.code_hash, Hash32::ZERO);
    assert_eq!(account.data_hash, Hash32::ZERO);
    assert_eq!(account.storage_root, Hash32::ZERO);
}

#[test]
fn disabled_user_account_cannot_send_public_transaction() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));
    state.account_mut(sender).flags.disabled = true;

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
                amount: 1,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("account_disabled"));
    assert_eq!(state.account(sender).unwrap().nonce, 0);
    assert!(state.account(recipient).is_none());
}

#[test]
fn contract_only_account_cannot_send_public_transaction() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let contract_sender = account(b"contract-sender");
    let recipient = account(b"recipient");
    assert!(state
        .account_mut(contract_sender)
        .credit(L2_NATIVE_GAS_ASSET, 100));
    let account = state.account_mut(contract_sender);
    account.account_type = AccountType::Contract;
    account.flags = AccountFlags {
        contract_only: true,
        ..AccountFlags::default()
    };

    let outcome = executor.apply(
        &mut state,
        &tx(
            contract_sender,
            0,
            10,
            1,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 1,
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("sender_contract_only")
    );
    assert_eq!(state.account(contract_sender).unwrap().nonce, 0);
    assert!(state.account(recipient).is_none());
}

#[test]
fn reserved_zero_address_rejects_deposit_transfer_and_deploy() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let sample = sample_counter_initial_state(0);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));

    let deposit = executor.apply(
        &mut state,
        &SignedL2Transaction::system_deposit(CHAIN_ID, account(b"deposit"), 0, Hash32::ZERO, 100),
        &ExecutionConfig::default(),
    );
    assert_eq!(deposit.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        deposit.receipt.reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert!(state.account(Hash32::ZERO).is_none());

    let transfer = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            1,
            L2TransactionKind::Transfer {
                to: Hash32::ZERO,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 10,
            },
        ),
        &ExecutionConfig::default(),
    );
    assert_eq!(transfer.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        transfer.receipt.reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert_eq!(transfer.receipt.gas_charged, 1);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert!(state.account(Hash32::ZERO).is_none());

    let deploy = executor.apply(
        &mut state,
        &tx(
            sender,
            1,
            GasSchedule::default().call_contract_gas,
            1,
            L2TransactionKind::DeployContract {
                contract: Hash32::ZERO,
                code_boc_base64: sample.code_boc_base64,
                data_boc_base64: sample.data_boc_base64,
            },
        ),
        &ExecutionConfig::default(),
    );
    assert_eq!(deploy.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        deploy.receipt.reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert_eq!(state.account(sender).unwrap().nonce, 2);
    assert!(state.account(Hash32::ZERO).is_none());
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
fn transfer_distributes_fee_to_configured_destinations() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    let sequencer = account(b"sequencer-reward");
    let operator = account(b"operator-fee");
    let treasury = account(b"treasury-fee");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            10,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 100,
            },
        ),
        &ExecutionConfig {
            block_height: 9,
            fee_accounting: FeeAccountingConfig {
                operator_commission_bps: 2_500,
                treasury_fee_bps: 1_000,
                sequencer_reward_account: sequencer,
                operator_fee_account: operator,
                treasury_fee_account: treasury,
            },
            ..ExecutionConfig::default()
        },
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 100);
    assert_eq!(state.account(sender).unwrap().balance(0), 800);
    assert_eq!(state.account(recipient).unwrap().balance(0), 100);
    assert_eq!(state.account(sequencer).unwrap().balance(0), 65);
    assert_eq!(state.account(operator).unwrap().balance(0), 25);
    assert_eq!(state.account(treasury).unwrap().balance(0), 10);
    assert_eq!(
        outcome.receipt.events,
        vec![L2Event::FeeDistributed {
            asset_id: L2_NATIVE_GAS_ASSET,
            total_amount: 100,
            sequencer_amount: 65,
            operator_amount: 25,
            treasury_amount: 10,
            sequencer_reward_account: sequencer,
            operator_fee_account: operator,
            treasury_fee_account: treasury,
        }]
    );
}

#[test]
fn rejected_attempt_distributes_rejection_fee() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    let sequencer = account(b"sequencer-reward");
    let operator = account(b"operator-fee");
    let treasury = account(b"treasury-fee");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            9,
            10,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 1,
            },
        ),
        &ExecutionConfig {
            block_height: 9,
            fee_accounting: FeeAccountingConfig {
                operator_commission_bps: 2_000,
                treasury_fee_bps: 1_000,
                sequencer_reward_account: sequencer,
                operator_fee_account: operator,
                treasury_fee_account: treasury,
            },
            ..ExecutionConfig::default()
        },
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("insufficient_gas_limit")
    );
    assert_eq!(outcome.receipt.gas_charged, 10);
    assert_eq!(state.account(sender).unwrap().balance(0), 90);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert_eq!(state.account(sequencer).unwrap().balance(0), 7);
    assert_eq!(state.account(operator).unwrap().balance(0), 2);
    assert_eq!(state.account(treasury).unwrap().balance(0), 1);
    assert_eq!(outcome.receipt.events.len(), 1);
    assert!(matches!(
        outcome.receipt.events[0],
        L2Event::FeeDistributed {
            total_amount: 10,
            sequencer_amount: 7,
            operator_amount: 2,
            treasury_amount: 1,
            ..
        }
    ));
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
fn withdraw_emits_deterministic_receipt_event() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let l1_recipient = "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR".to_owned();
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            20,
            1,
            L2TransactionKind::Withdraw {
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 100,
                l1_recipient: l1_recipient.clone(),
            },
        ),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    let withdrawal = outcome.withdrawals.first().expect("withdrawal leaf");
    assert_eq!(outcome.receipt.events.len(), 2);
    assert_eq!(
        outcome.receipt.events[0],
        L2Event::WithdrawalCreated {
            withdrawal_id: withdrawal.withdrawal_id,
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 100,
            l2_sender: sender,
            l1_recipient,
        }
    );
    assert!(matches!(
        outcome.receipt.events[1],
        L2Event::FeeDistributed {
            total_amount: 20,
            ..
        }
    ));
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
