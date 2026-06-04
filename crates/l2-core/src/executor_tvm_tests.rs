use super::*;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::State;
use crate::types::{L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET};
use crate::{
    GasSchedule, TvmAdapterError, TvmExecutionAdapter, TvmExecutionInput, TvmExecutionOutput,
    TvmExecutionStatus, TvmInternalMessage, TvmStateDelta,
};
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

fn valid_boc_base64() -> String {
    let cell = CellBuilder::new().build().expect("empty cell");
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
    let contract_account = state.account_mut(contract);
    contract_account.code_hash = account(b"contract-code");
    contract_account.data_hash = account(b"old-data");
    contract_account.storage_root = account(b"old-storage");
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
fn call_contract_charges_rejection_fee_until_real_adapter_exists() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    fund_sender_and_contract(&mut state, sender, contract, 100);

    let outcome = executor.apply(
        &mut state,
        &call_tx(sender, contract, valid_boc_base64()),
        &ExecutionConfig::default(),
    );

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("tvm_adapter_not_implemented")
    );
    assert_eq!(outcome.receipt.gas_charged, 2);
    assert_eq!(state.account(sender).unwrap().balance(0), 98);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn mock_adapter_applies_contract_delta_and_returns_internal_messages() {
    let executor = DeterministicExecutor;
    let mut state = State::default();
    let sender = account(b"sender");
    let contract = account(b"contract");
    let new_storage = account(b"new-storage");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            data_hash: Some(account(b"new-data")),
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
    assert_eq!(state.account(contract).unwrap().storage_root, new_storage);
    assert_eq!(state.account(contract).unwrap().last_lt, 7);
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
    let old_storage = account(b"old-storage");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            data_hash: None,
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
    let old_storage = account(b"old-storage");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract,
            code_hash: None,
            data_hash: None,
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
    let old_storage = account(b"old-storage");
    fund_sender_and_contract(&mut state, sender, contract, 1_000);
    let adapter = MockTvmAdapter {
        output: Ok(TvmExecutionOutput::applied(
            10,
            Some(TvmStateDelta {
                contract: account(b"other-contract"),
                code_hash: None,
                data_hash: None,
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
            data_hash: Some(account(b"new-data")),
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
