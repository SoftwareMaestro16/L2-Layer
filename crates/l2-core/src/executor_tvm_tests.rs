use super::*;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::State;
use crate::types::{L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET};
use crate::{
    read_sample_counter_value, sample_counter_initial_state, sample_counter_storage_root,
    GasSchedule, TvmAdapterError, TvmExecutionAdapter, TvmExecutionInput, TvmExecutionOutput,
    TvmExecutionStatus, TvmInternalMessage, TvmStateDelta, SAMPLE_COUNTER_INCREMENT_OPCODE,
};
use crate::{L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::{BagOfCells, CellBuilder};

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
        tvm_adapter_mode: TvmAdapterMode::Prototype,
        ..ExecutionConfig::default()
    }
}

fn valid_boc_base64() -> String {
    let cell = CellBuilder::new().build().expect("empty cell");
    let boc = BagOfCells::from_root(cell)
        .serialize(false)
        .expect("serialize boc");
    BASE64_STANDARD.encode(boc)
}

fn boc_hash(boc_base64: &str) -> Hash32 {
    let boc = BASE64_STANDARD
        .decode(boc_base64.as_bytes())
        .expect("valid test BoC");
    crate::boc_single_root_hash(&boc).expect("valid single-root test BoC")
}

fn sample_increment_boc_base64(increment: u32) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, SAMPLE_COUNTER_INCREMENT_OPCODE)
        .expect("store opcode")
        .store_u32(32, increment)
        .expect("store increment");
    let cell = builder.build().expect("increment cell");
    let boc = BagOfCells::from_root(cell)
        .serialize(false)
        .expect("serialize boc");
    BASE64_STANDARD.encode(boc)
}

fn call_tx(from: Hash32, contract: Hash32, body_boc_base64: String) -> SignedL2Transaction {
    tx(
        from,
        0,
        50,
        2,
        L2TransactionKind::CallContract {
            contract,
            body_boc_base64,
        },
    )
}

fn fund_sender_and_contract(
    state: &mut State,
    sender: Hash32,
    contract: Hash32,
    gas_balance: u128,
) {
    assert!(state
        .account_mut(sender)
        .credit(L2_NATIVE_GAS_ASSET, gas_balance));
    let code_boc_base64 = valid_boc_base64();
    let data_boc_base64 = valid_boc_base64();
    let code_hash = boc_hash(&code_boc_base64);
    let data_hash = boc_hash(&data_boc_base64);
    let contract_account = state.account_mut(contract);
    contract_account.code_hash = code_hash;
    contract_account.data_hash = data_hash;
    contract_account.storage_root = data_hash;
    contract_account.code_boc_base64 = Some(code_boc_base64);
    contract_account.data_boc_base64 = Some(data_boc_base64);
}

fn fund_sender_and_sample_counter(
    state: &mut State,
    sender: Hash32,
    contract: Hash32,
    gas_balance: u128,
    initial_counter: u64,
) {
    assert!(state
        .account_mut(sender)
        .credit(L2_NATIVE_GAS_ASSET, gas_balance));
    let sample = sample_counter_initial_state(initial_counter);
    let contract_account = state.account_mut(contract);
    contract_account.code_hash = sample.code_hash;
    contract_account.data_hash = sample.data_hash;
    contract_account.storage_root = sample.storage_root;
    contract_account.code_boc_base64 = Some(sample.code_boc_base64);
    contract_account.data_boc_base64 = Some(sample.data_boc_base64);
}

#[derive(Clone)]
struct MockTvmAdapter {
    output: Result<TvmExecutionOutput, TvmAdapterError>,
}

impl TvmExecutionAdapter for MockTvmAdapter {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        assert_eq!(input.gas_limit, 50);
        assert_eq!(input.context.block_height, 7);
        assert_eq!(input.context.gas_coin_asset, L2_NATIVE_GAS_ASSET);
        self.output.clone()
    }
}

#[test]
fn call_contract_rejects_malformed_boc_before_adapter() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 100);

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, String::new()),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("malformed_boc"));
    assert_eq!(outcome.receipt.gas_charged, 2);
    assert_eq!(state.account(sender).unwrap().balance(0), 98);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn call_contract_rejects_reserved_zero_contract() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 100));

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, Hash32::ZERO, valid_boc_base64()),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert_eq!(outcome.receipt.gas_charged, 2);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert!(state.account(Hash32::ZERO).is_none());
}

#[test]
fn call_contract_rejects_oversized_encoded_boc_before_decode() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 100);
    let config = ExecutionConfig {
        max_tvm_boc_bytes: 1,
        ..ExecutionConfig::default()
    };

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, "AAAAAAAA".to_owned()),
        &config,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("boc_too_large"));
    assert_eq!(outcome.receipt.gas_charged, 2);
    assert_eq!(state.account(sender).unwrap().balance(0), 98);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn call_contract_real_adapter_missing_library_fails_closed() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 100);
    let config = ExecutionConfig {
        tvm_tonlib_library_path: Some("__missing_tonlibjson_for_test__".into()),
        ..ExecutionConfig::default()
    };

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("tvm_adapter_failed")
    );
    assert_eq!(outcome.receipt.gas_charged, 2);
    assert_eq!(state.account(sender).unwrap().balance(0), 98);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn prototype_adapter_applies_sample_counter_increment() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    fund_sender_and_sample_counter(&mut state, sender, contract, 1_000, 4);

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, sample_increment_boc_base64(3)),
        &config(GasSchedule::default()),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 50);
    assert_eq!(
        read_sample_counter_value(state.account(contract).unwrap()),
        Ok(7)
    );
    assert_eq!(state.account(sender).unwrap().balance(0), 950);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn ent_fees_are_charged_for_transfer_deploy_and_call_actions() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let recipient = account(b"recipient");
    let contract = account(b"sample-counter");
    let initial = sample_counter_initial_state(0);
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));
    assert!(state.account_mut(sender).credit(2, 50));

    let transfer = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            3,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: 2,
                amount: 40,
            },
        ),
        &ExecutionConfig::default(),
    );
    assert_eq!(transfer.receipt.status, ReceiptStatus::Applied);
    assert_eq!(transfer.receipt.gas_charged, 30);
    assert_eq!(
        state.account(sender).unwrap().balance(L2_NATIVE_GAS_ASSET),
        970
    );
    assert_eq!(state.account(sender).unwrap().balance(2), 10);
    assert_eq!(state.account(recipient).unwrap().balance(2), 40);

    let deploy = executor.apply(
        &mut state,
        &tx(
            sender,
            1,
            50,
            2,
            L2TransactionKind::DeployContract {
                contract,
                code_boc_base64: initial.code_boc_base64,
                data_boc_base64: initial.data_boc_base64,
            },
        ),
        &ExecutionConfig::default(),
    );
    assert_eq!(deploy.receipt.status, ReceiptStatus::Applied);
    assert_eq!(deploy.receipt.gas_charged, 100);
    assert_eq!(
        state.account(sender).unwrap().balance(L2_NATIVE_GAS_ASSET),
        870
    );

    let call = executor.apply(
        &mut state,
        &tx(
            sender,
            2,
            50,
            4,
            L2TransactionKind::CallContract {
                contract,
                body_boc_base64: sample_increment_boc_base64(1),
            },
        ),
        &config(GasSchedule::default()),
    );
    assert_eq!(call.receipt.status, ReceiptStatus::Applied);
    assert_eq!(call.receipt.gas_charged, 100);
    assert_eq!(
        state.account(sender).unwrap().balance(L2_NATIVE_GAS_ASSET),
        770
    );
    assert_eq!(state.account(sender).unwrap().nonce, 3);
    assert_eq!(
        read_sample_counter_value(state.account(contract).unwrap()),
        Ok(1)
    );
}

#[test]
fn prototype_adapter_replays_sample_counter_deterministically() {
    let executor = DeterministicExecutor;
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    let tx = call_tx(sender, contract, sample_increment_boc_base64(2));
    let config = config(GasSchedule::default());

    let mut first = State::default();
    fund_sender_and_sample_counter(&mut first, sender, contract, 1_000, 10);
    let first_outcome = executor.apply(&mut first, &tx, &config);

    let mut second = State::default();
    fund_sender_and_sample_counter(&mut second, sender, contract, 1_000, 10);
    let second_outcome = executor.apply(&mut second, &tx, &config);

    assert_eq!(first_outcome.receipt, second_outcome.receipt);
    assert_eq!(first.root_hash(), second.root_hash());
    assert_eq!(
        read_sample_counter_value(first.account(contract).unwrap()),
        Ok(12)
    );
}

#[test]
fn prototype_adapter_rejects_sample_counter_gas_exhaustion() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    fund_sender_and_sample_counter(&mut state, sender, contract, 1_000, 4);
    let schedule = GasSchedule {
        call_contract_gas: 1,
        ..GasSchedule::default()
    };

    let outcome = executor.apply(
        &mut state,
        &tx(
            sender,
            0,
            10,
            2,
            L2TransactionKind::CallContract {
                contract,
                body_boc_base64: sample_increment_boc_base64(1),
            },
        ),
        &config(schedule),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("gas_exhausted"));
    assert_eq!(outcome.receipt.gas_charged, 20);
    assert_eq!(
        read_sample_counter_value(state.account(contract).unwrap()),
        Ok(4)
    );
}

#[test]
fn prototype_adapter_rejects_corrupted_sample_counter_storage() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"sample-counter");
    fund_sender_and_sample_counter(&mut state, sender, contract, 1_000, 4);
    state.account_mut(contract).storage_root = sample_counter_storage_root(5);

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, sample_increment_boc_base64(1)),
        &config(GasSchedule::default()),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("sample_counter_bad_storage")
    );
    assert_eq!(
        state.account(contract).unwrap().storage_root,
        sample_counter_storage_root(5)
    );
}

#[test]
fn mock_adapter_applies_contract_delta_and_returns_internal_messages() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    let new_storage = account(b"new-storage");
    let new_data_boc = valid_boc_base64();
    let new_data_hash = boc_hash(&new_data_boc);
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: Some(new_data_hash),
            data_boc_base64: Some(new_data_boc.clone()),
            storage_root: Some(new_storage),
        }),
        emitted_internal_messages: vec![TvmInternalMessage {
            from: contract,
            to: account(b"receiver"),
            value: 0,
            body_boc: vec![1, 2, 3],
        }],
        gas_used: 11,
    };
    let adapter = MockTvmAdapter { output: Ok(output) };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config(GasSchedule::default()),
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.receipt.gas_charged, 22);
    assert_eq!(outcome.internal_messages.len(), 1);
    assert_eq!(state.account(sender).unwrap().balance(0), 978);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
    assert_eq!(state.account(contract).unwrap().data_hash, new_data_hash);
    assert_eq!(
        state.account(contract).unwrap().data_boc_base64.as_deref(),
        Some(new_data_boc.as_str())
    );
    assert_eq!(state.account(contract).unwrap().storage_root, new_storage);
    assert_eq!(state.account(contract).unwrap().last_lt, 7);
}

#[test]
fn adapter_state_delta_hash_without_boc_is_rejected() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let original = state.account(contract).unwrap().clone();
    let adapter = MockTvmAdapter {
        output: Ok(TvmExecutionOutput {
            status: TvmExecutionStatus::Applied,
            state_delta: Some(TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: Some(account(b"hash-without-cell")),
                data_boc_base64: None,
                storage_root: Some(account(b"new-storage")),
            }),
            emitted_internal_messages: vec![],
            gas_used: 11,
        }),
    };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config(GasSchedule::default()),
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("tvm_state_delta_data_hash_mismatch")
    );
    assert_eq!(state.account(contract).unwrap(), &original);
}

#[test]
fn mock_adapter_rejected_execution_charges_reported_gas() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let adapter = MockTvmAdapter {
        output: Ok(TvmExecutionOutput::rejected(50, "gas_exhausted")),
    };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config(GasSchedule::default()),
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(outcome.receipt.reason.as_deref(), Some("gas_exhausted"));
    assert_eq!(outcome.receipt.gas_charged, 100);
    assert_eq!(state.account(sender).unwrap().balance(0), 900);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn adapter_oversized_internal_message_body_is_rejected_without_delta() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let old_storage = state.account(contract).unwrap().storage_root;
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: None,
            data_boc_base64: None,
            storage_root: Some(account(b"new-storage")),
        }),
        emitted_internal_messages: vec![TvmInternalMessage {
            from: contract,
            to: account(b"receiver"),
            value: 0,
            body_boc: vec![0; crate::DEFAULT_MAX_TVM_BOC_BYTES + 1],
        }],
        gas_used: 10,
    };
    let adapter = MockTvmAdapter { output: Ok(output) };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config(GasSchedule::default()),
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("internal_message_boc_too_large")
    );
    assert_eq!(outcome.receipt.gas_charged, 20);
    assert_eq!(state.account(contract).unwrap().storage_root, old_storage);
}

#[test]
fn adapter_output_over_internal_message_limit_is_rejected_without_delta() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let old_storage = state.account(contract).unwrap().storage_root;
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: None,
            data_boc_base64: None,
            storage_root: Some(account(b"new-storage")),
        }),
        emitted_internal_messages: vec![
            TvmInternalMessage {
                from: contract,
                to: account(b"a"),
                value: 0,
                body_boc: vec![],
            },
            TvmInternalMessage {
                from: contract,
                to: account(b"b"),
                value: 0,
                body_boc: vec![],
            },
        ],
        gas_used: 10,
    };
    let adapter = MockTvmAdapter { output: Ok(output) };
    let config = ExecutionConfig {
        max_internal_messages: 1,
        ..config(GasSchedule::default())
    };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config,
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("too_many_internal_messages")
    );
    assert_eq!(outcome.receipt.gas_charged, 20);
    assert_eq!(state.account(contract).unwrap().storage_root, old_storage);
}

#[test]
fn adapter_state_delta_for_wrong_contract_is_rejected_without_storage_corruption() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let old_storage = state.account(contract).unwrap().storage_root;
    let adapter = MockTvmAdapter {
        output: Ok(TvmExecutionOutput::applied(
            10,
            Some(TvmStateDelta {
                contract: account(b"other-contract"),
                code_hash: None,
                code_boc_base64: None,
                data_hash: None,
                data_boc_base64: None,
                storage_root: Some(account(b"attacker-storage")),
            }),
        )),
    };

    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &config(GasSchedule::default()),
        &adapter,
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("tvm_state_delta_contract_mismatch")
    );
    assert_eq!(state.account(contract).unwrap().storage_root, old_storage);
    assert!(state.account(account(b"other-contract")).is_none());
}

#[test]
fn mock_adapter_replay_is_deterministic_for_same_input() {
    let executor = DeterministicExecutor;
    let sender = account(b"sender");
    let contract = account(b"contract");
    let output = TvmExecutionOutput::applied(
        12,
        Some(TvmStateDelta {
            contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: Some(account(b"new-data")),
            data_boc_base64: None,
            storage_root: Some(account(b"new-storage")),
        }),
    );
    let adapter = MockTvmAdapter { output: Ok(output) };
    let tx = call_tx(sender, contract, valid_boc_base64());
    let config = config(GasSchedule::default());

    let mut first = State::default();
    fund_sender_and_contract(&mut first, sender, contract, 1_000);
    let first_outcome = executor.apply_with_tvm_adapter(&mut first, &tx, &config, &adapter);

    let mut second = State::default();
    fund_sender_and_contract(&mut second, sender, contract, 1_000);
    let second_outcome = executor.apply_with_tvm_adapter(&mut second, &tx, &config, &adapter);

    assert_eq!(first_outcome.receipt, second_outcome.receipt);
    assert_eq!(first.root_hash(), second.root_hash());
}
