use super::*;
use crate::crypto::{sha256_bytes, Hash32};
use crate::state::State;
use crate::tvm::{
    TvmAdapterError, TvmExecutionAdapter, TvmExecutionInput, TvmExecutionOutput,
    TvmExecutionStatus, TvmInternalMessage, TvmStateDelta,
};
use crate::types::{
    L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::{BagOfCells, CellBuilder};

const CHAIN_ID: &str = "entropis-testnet";

#[derive(Clone)]
struct FixedAdapter {
    output: TvmExecutionOutput,
}

impl TvmExecutionAdapter for FixedAdapter {
    fn execute(&self, _input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        Ok(self.output.clone())
    }
}

fn account(seed: &[u8]) -> Hash32 {
    sha256_bytes(seed)
}

fn valid_boc_base64() -> String {
    let cell = CellBuilder::new().build().expect("empty cell");
    BASE64_STANDARD.encode(
        BagOfCells::from_root(cell)
            .serialize(false)
            .expect("serialize boc"),
    )
}

fn larger_boc_base64() -> String {
    let mut builder = CellBuilder::new();
    for _ in 0..512 {
        builder.store_bit(false).expect("store bit");
    }
    BASE64_STANDARD.encode(
        BagOfCells::from_root(builder.build().expect("large cell"))
            .serialize(false)
            .expect("serialize large boc"),
    )
}

fn decoded_boc_len(boc_base64: &str) -> usize {
    BASE64_STANDARD
        .decode(boc_base64.as_bytes())
        .expect("valid test BoC")
        .len()
}

fn multi_root_boc_base64() -> String {
    let first = CellBuilder::new().build().expect("first cell");
    let second = CellBuilder::new()
        .store_u32(32, 0xfeed_face)
        .expect("store second")
        .build()
        .expect("second cell");
    let mut boc = BagOfCells::from_root(first);
    boc.add_root(second);
    BASE64_STANDARD.encode(boc.serialize(false).expect("serialize multi-root boc"))
}

fn boc_hash(boc_base64: &str) -> Hash32 {
    let boc = BASE64_STANDARD
        .decode(boc_base64.as_bytes())
        .expect("valid test BoC");
    crate::boc_single_root_hash(&boc).expect("valid single-root test BoC")
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

fn call_tx(from: Hash32, contract: Hash32) -> SignedL2Transaction {
    tx(
        from,
        0,
        50,
        2,
        L2TransactionKind::CallContract {
            contract,
            body_boc_base64: valid_boc_base64(),
        },
    )
}

fn deploy_tx(
    from: Hash32,
    contract: Hash32,
    code_boc_base64: String,
    data_boc_base64: String,
) -> SignedL2Transaction {
    tx(
        from,
        0,
        50,
        2,
        L2TransactionKind::DeployContract {
            contract,
            code_boc_base64,
            data_boc_base64,
        },
    )
}

fn funded_state_with_contract() -> (State, Hash32, Hash32, Hash32) {
    let sender = account(b"sender");
    let contract = account(b"contract");
    let mut state = State::default();
    assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));
    let code_boc_base64 = valid_boc_base64();
    let data_boc_base64 = valid_boc_base64();
    let storage_root = boc_hash(&data_boc_base64);
    let contract_account = state.account_mut(contract);
    contract_account.code_hash = boc_hash(&code_boc_base64);
    contract_account.data_hash = storage_root;
    contract_account.storage_root = storage_root;
    contract_account.code_boc_base64 = Some(code_boc_base64);
    contract_account.data_boc_base64 = Some(data_boc_base64);
    (state, sender, contract, storage_root)
}

fn apply_with_output(output: TvmExecutionOutput) -> (ExecutionOutcome, State, Hash32, Hash32) {
    let executor = DeterministicExecutor;
    let (mut state, sender, contract, _) = funded_state_with_contract();
    let adapter = FixedAdapter { output };
    let outcome = executor.apply_with_tvm_adapter(
        &mut state,
        &call_tx(sender, contract),
        &ExecutionConfig::default(),
        &adapter,
    );
    (outcome, state, sender, contract)
}

fn output_with_delta(_contract: Hash32, gas_used: u64, delta: TvmStateDelta) -> TvmExecutionOutput {
    TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(delta),
        emitted_internal_messages: vec![],
        gas_used,
    }
}

#[test]
fn deploy_rejects_empty_and_multi_root_contract_bocs() {
    let executor = DeterministicExecutor;
    let sender = account(b"sender");
    let contract = account(b"contract");
    let valid = valid_boc_base64();
    let multi_root = multi_root_boc_base64();

    for (code, data, reason) in [
        ("".to_owned(), valid.clone(), "malformed_code_boc"),
        (valid.clone(), "".to_owned(), "malformed_data_boc"),
        (multi_root.clone(), valid.clone(), "malformed_code_boc"),
        (valid, multi_root, "malformed_data_boc"),
    ] {
        let mut state = State::default();
        assert!(state.account_mut(sender).credit(L2_NATIVE_GAS_ASSET, 1_000));

        let outcome = executor.apply(
            &mut state,
            &deploy_tx(sender, contract, code, data),
            &ExecutionConfig::default(),
        );

        assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
        assert_eq!(outcome.receipt.reason.as_deref(), Some(reason));
        assert!(state
            .account(contract)
            .is_none_or(|account| account.code_hash == Hash32::ZERO));
    }
}

#[test]
fn adapter_zero_or_excessive_gas_is_rejected_without_state_delta() {
    let contract = account(b"contract");
    for (gas_used, reason) in [(0, "tvm_zero_gas_used"), (51, "tvm_gas_used_exceeds_limit")] {
        let new_data_boc = valid_boc_base64();
        let (outcome, state, _sender, contract) = apply_with_output(output_with_delta(
            contract,
            gas_used,
            TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: Some(boc_hash(&new_data_boc)),
                data_boc_base64: Some(new_data_boc),
                storage_root: Some(account(b"attacker-storage")),
            },
        ));

        assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
        assert_eq!(outcome.receipt.reason.as_deref(), Some(reason));
        assert_ne!(
            state.account(contract).unwrap().storage_root,
            account(b"attacker-storage")
        );
    }
}

#[test]
fn adapter_invalid_rejected_reason_is_sanitized() {
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Rejected {
            reason: "Bad-Reason".to_owned(),
        },
        state_delta: None,
        emitted_internal_messages: vec![],
        gas_used: 10,
    };

    let (outcome, state, sender, _contract) = apply_with_output(output);

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("invalid_tvm_receipt_reason")
    );
    assert_eq!(outcome.receipt.gas_charged, 20);
    assert_eq!(state.account(sender).unwrap().nonce, 1);
}

#[test]
fn adapter_internal_messages_must_match_source_and_nonzero_addresses() {
    let contract = account(b"contract");
    for (from, to, reason) in [
        (
            account(b"wrong-source"),
            account(b"receiver"),
            "internal_message_source_mismatch",
        ),
        (contract, Hash32::ZERO, "reserved_zero_address"),
    ] {
        let (outcome, state, _sender, contract) = apply_with_output(TvmExecutionOutput {
            status: TvmExecutionStatus::Applied,
            state_delta: Some(TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: None,
                data_boc_base64: None,
                storage_root: Some(account(b"attacker-storage")),
            }),
            emitted_internal_messages: vec![TvmInternalMessage {
                from,
                to,
                value: 0,
                body_boc: vec![],
                bounce: true,
                bounced: false,
            }],
            gas_used: 10,
        });

        assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
        assert_eq!(outcome.receipt.reason.as_deref(), Some(reason));
        assert!(outcome.internal_messages.is_empty());
        assert_ne!(
            state.account(contract).unwrap().storage_root,
            account(b"attacker-storage")
        );
    }
}

#[test]
fn adapter_exact_internal_message_limit_is_accepted() {
    let contract = account(b"contract");
    let output = TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: None,
        emitted_internal_messages: vec![TvmInternalMessage {
            from: contract,
            to: account(b"receiver"),
            value: 0,
            body_boc: vec![],
            bounce: true,
            bounced: false,
        }],
        gas_used: 10,
    };
    let executor = DeterministicExecutor;
    let (mut state, sender, contract, _) = funded_state_with_contract();
    let adapter = FixedAdapter { output };
    let config = ExecutionConfig {
        max_internal_messages: 1,
        ..ExecutionConfig::default()
    };

    let outcome =
        executor.apply_with_tvm_adapter(&mut state, &call_tx(sender, contract), &config, &adapter);

    assert_eq!(outcome.receipt.status, ReceiptStatus::Applied);
    assert_eq!(outcome.internal_messages.len(), 1);
}

#[test]
fn adapter_delta_bocs_must_match_declared_hashes_and_be_well_formed() {
    let contract = account(b"contract");
    let valid = valid_boc_base64();
    for (delta, reason) in [
        (
            TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: Some(account(b"wrong-data-hash")),
                data_boc_base64: Some(valid.clone()),
                storage_root: Some(account(b"attacker-storage")),
            },
            "tvm_state_delta_data_hash_mismatch",
        ),
        (
            TvmStateDelta {
                contract,
                code_hash: Some(account(b"wrong-code-hash")),
                code_boc_base64: Some(valid.clone()),
                data_hash: None,
                data_boc_base64: None,
                storage_root: Some(account(b"attacker-storage")),
            },
            "tvm_state_delta_code_hash_mismatch",
        ),
        (
            TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: Some(account(b"hash")),
                data_boc_base64: Some("***not-base64***".to_owned()),
                storage_root: Some(account(b"attacker-storage")),
            },
            "tvm_state_delta_malformed_cell_boc",
        ),
    ] {
        let (outcome, state, _sender, contract) =
            apply_with_output(output_with_delta(contract, 10, delta));

        assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
        assert_eq!(outcome.receipt.reason.as_deref(), Some(reason));
        assert_ne!(
            state.account(contract).unwrap().storage_root,
            account(b"attacker-storage")
        );
    }
}

#[test]
fn adapter_delta_cell_size_limit_is_enforced() {
    let call_body = valid_boc_base64();
    let large_delta = larger_boc_base64();
    let executor = DeterministicExecutor;
    let (mut state, sender, contract, _) = funded_state_with_contract();
    let adapter = FixedAdapter {
        output: output_with_delta(
            contract,
            10,
            TvmStateDelta {
                contract,
                code_hash: None,
                code_boc_base64: None,
                data_hash: Some(boc_hash(&large_delta)),
                data_boc_base64: Some(large_delta),
                storage_root: Some(account(b"attacker-storage")),
            },
        ),
    };
    let config = ExecutionConfig {
        max_tvm_boc_bytes: decoded_boc_len(&call_body),
        ..ExecutionConfig::default()
    };

    let outcome =
        executor.apply_with_tvm_adapter(&mut state, &call_tx(sender, contract), &config, &adapter);

    assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
    assert_eq!(
        outcome.receipt.reason.as_deref(),
        Some("tvm_state_delta_cell_boc_too_large")
    );
    assert_ne!(
        state.account(contract).unwrap().storage_root,
        account(b"attacker-storage")
    );
}
