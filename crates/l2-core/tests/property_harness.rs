use l2_core::crypto::sha256_bytes;
use l2_core::validate_tvm_output;
use l2_core::{
    canonical_batch_data_bytes, decode_batch_data, BatchBuildInput, BatchBuilder, Hash32, L2Event,
    L2TransactionKind, Receipt, SignedL2Transaction, TvmBoundaryError, TvmExecutionOutput,
    TvmExecutionStatus, TvmInternalMessage, TvmStateDelta, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use std::collections::BTreeSet;

const CHAIN_ID: &str = "entropis-property-testnet";

#[test]
fn transaction_field_mutations_are_hash_bound_except_auth_fields() {
    let base = transfer_tx(0);

    assert_hash_bound("tx_version", &base, |tx| tx.tx_version += 1);
    assert_hash_bound("domain_separator", &base, |tx| {
        tx.domain_separator = "entropis.l2.tx.v3".to_owned();
    });
    assert_hash_bound("chain_id", &base, |tx| {
        tx.chain_id = "entropis-other-testnet".to_owned();
    });
    assert_hash_bound("from", &base, |tx| tx.from = Some(hash(0x41)));
    assert_hash_bound("nonce", &base, |tx| tx.nonce += 1);
    assert_hash_bound("valid_until_block", &base, |tx| tx.valid_until_block -= 1);
    assert_hash_bound("gas_limit", &base, |tx| tx.gas_limit += 1);
    assert_hash_bound("max_gas_price", &base, |tx| tx.max_gas_price += 1);
    assert_hash_bound("fee_asset_id", &base, |tx| tx.fee_asset_id += 1);
    assert_hash_bound("memo_hash", &base, |tx| tx.memo_hash = Some(hash(0x42)));
    assert_hash_bound("kind_version", &base, |tx| {
        tx.transaction_kind_version += 1;
    });
    assert_hash_bound("transfer_to", &base, |tx| {
        tx.kind = L2TransactionKind::Transfer {
            to: hash(0x43),
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 10,
        };
    });
    assert_hash_bound("transfer_asset", &base, |tx| {
        tx.kind = L2TransactionKind::Transfer {
            to: hash(0x20),
            asset_id: 7,
            amount: 10,
        };
    });
    assert_hash_bound("transfer_amount", &base, |tx| {
        tx.kind = L2TransactionKind::Transfer {
            to: hash(0x20),
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 11,
        };
    });

    let mut auth_mutation = base.clone();
    auth_mutation.public_key = Some("aa".repeat(32));
    auth_mutation.signature = Some("bb".repeat(64));

    assert_eq!(base.tx_hash(), auth_mutation.tx_hash());
    assert_eq!(base.signing_payload(), auth_mutation.signing_payload());
}

#[test]
fn deterministic_transaction_mutation_corpus_has_unique_hashes() {
    let mut seen = BTreeSet::new();

    for seed in 0..128 {
        let tx = transfer_tx(seed);
        assert!(
            seen.insert(tx.tx_hash()),
            "duplicate tx hash at seed {seed}"
        );
    }
}

#[test]
fn batch_mutation_corpus_is_deterministic_and_order_sensitive() {
    for len in 1..=16 {
        let txs = (0..len).map(deposit_tx).collect::<Vec<_>>();
        let receipts = txs
            .iter()
            .map(|tx| Receipt::applied(tx.tx_hash(), 0, None))
            .collect::<Vec<_>>();
        let input = batch_input(txs.clone(), receipts.clone(), 100 + len as u64);

        let first = BatchBuilder::build(input.clone()).expect("first batch");
        let second = BatchBuilder::build(input).expect("second batch");

        assert_eq!(first.header.block_hash(), second.header.block_hash());
        assert_eq!(first.header.data_hash, second.header.data_hash);

        let mut reversed_txs = txs;
        let mut reversed_receipts = receipts;
        reversed_txs.reverse();
        reversed_receipts.reverse();
        let reversed = BatchBuilder::build(batch_input(
            reversed_txs,
            reversed_receipts,
            100 + len as u64,
        ))
        .expect("reversed batch");

        if len > 1 {
            assert_ne!(first.header.tx_root, reversed.header.tx_root);
            assert_ne!(first.header.data_hash, reversed.header.data_hash);
            assert_ne!(first.header.block_hash(), reversed.header.block_hash());
        }

        let timestamp_mutation = BatchBuilder::build(batch_input(
            first.transactions.clone(),
            first.receipts.clone(),
            999,
        ))
        .expect("timestamp mutation");
        assert_ne!(
            first.header.block_hash(),
            timestamp_mutation.header.block_hash()
        );
    }
}

#[test]
fn canonical_batch_data_decode_has_no_silent_alias_for_seeded_byte_mutations() {
    let txs = (0..8).map(deposit_tx).collect::<Vec<_>>();
    let receipts = txs
        .iter()
        .map(|tx| {
            Receipt::applied(tx.tx_hash(), 0, None).with_events(vec![L2Event::ContractCalled {
                contract: hash(0x55),
                caller: hash(0x56),
                body_hash: hash(0x57),
            }])
        })
        .collect::<Vec<_>>();
    let canonical = canonical_batch_data_bytes(&txs, &receipts);

    let decoded = decode_batch_data(&canonical).expect("canonical payload");
    assert_eq!(decoded.transactions, txs);
    assert_eq!(decoded.receipts, receipts);

    for seed in 0..32 {
        let mut mutated = canonical.clone();
        let index = seeded_index(seed, mutated.len());
        mutated[index] ^= seeded_byte(seed);
        if let Ok(decoded) = decode_batch_data(&mutated) {
            assert!(
                decoded.transactions != txs || decoded.receipts != receipts,
                "mutated batch payload silently aliased canonical data at seed {seed}"
            );
            assert_eq!(
                canonical_batch_data_bytes(&decoded.transactions, &decoded.receipts),
                mutated,
                "decoded mutation is not canonical at seed {seed}"
            );
        }
    }

    for len in 0..canonical.len().min(32) {
        assert!(
            decode_batch_data(&canonical[..len]).is_err(),
            "truncated batch payload decoded at len {len}"
        );
    }
}

#[test]
fn tvm_output_boundary_mutation_corpus_rejects_invalid_adapter_outputs() {
    let contract = hash(0x60);
    let mut output = TvmExecutionOutput::applied(
        10,
        Some(TvmStateDelta {
            contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: None,
            data_boc_base64: None,
            storage_root: Some(hash(0x61)),
        }),
    );

    validate_tvm_output(&output, contract, 10, 2, 4).expect("valid base output");

    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| mutated.gas_used = 0,
        TvmBoundaryError::ZeroGasUsed,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| mutated.gas_used = 11,
        TvmBoundaryError::GasUsedExceedsLimit,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| {
            mutated.emitted_internal_messages = vec![
                internal_message(contract, hash(0x62), &[1]),
                internal_message(contract, hash(0x63), &[2]),
                internal_message(contract, hash(0x64), &[3]),
            ];
        },
        TvmBoundaryError::TooManyInternalMessages,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| {
            mutated.emitted_internal_messages =
                vec![internal_message(contract, hash(0x62), &[1, 2, 3, 4, 5])];
        },
        TvmBoundaryError::InternalMessageBocTooLarge,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| {
            mutated.emitted_internal_messages =
                vec![internal_message(hash(0x65), hash(0x62), &[1])];
        },
        TvmBoundaryError::InternalMessageSourceMismatch,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| {
            mutated.emitted_internal_messages =
                vec![internal_message(contract, Hash32::ZERO, &[1])];
        },
        TvmBoundaryError::InternalMessageReservedAddress,
    );
    assert_tvm_boundary_error(
        &output,
        contract,
        |mutated| {
            mutated.state_delta = Some(TvmStateDelta {
                contract: hash(0x66),
                code_hash: None,
                code_boc_base64: None,
                data_hash: None,
                data_boc_base64: None,
                storage_root: Some(hash(0x61)),
            });
        },
        TvmBoundaryError::StateDeltaContractMismatch,
    );

    output.status = TvmExecutionStatus::Rejected {
        reason: "Bad Reason".to_owned(),
    };
    assert_eq!(
        validate_tvm_output(&output, contract, 10, 2, 4),
        Err(TvmBoundaryError::InvalidReceiptReason)
    );
}

fn assert_hash_bound(
    name: &str,
    base: &SignedL2Transaction,
    mutate: impl FnOnce(&mut SignedL2Transaction),
) {
    let mut mutated = base.clone();
    mutate(&mut mutated);

    assert_ne!(
        base.tx_hash(),
        mutated.tx_hash(),
        "{name} did not change tx_hash"
    );
    assert_ne!(
        base.signing_payload(),
        mutated.signing_payload(),
        "{name} did not change signing payload"
    );
}

fn assert_tvm_boundary_error(
    base: &TvmExecutionOutput,
    contract: Hash32,
    mutate: impl FnOnce(&mut TvmExecutionOutput),
    expected: TvmBoundaryError,
) {
    let mut mutated = base.clone();
    mutate(&mut mutated);
    assert_eq!(
        validate_tvm_output(&mutated, contract, 10, 2, 4),
        Err(expected)
    );
}

fn batch_input(
    ordered_transactions: Vec<SignedL2Transaction>,
    receipts: Vec<Receipt>,
    timestamp: u64,
) -> BatchBuildInput {
    BatchBuildInput {
        previous_header: None,
        prev_state_root: Hash32::ZERO,
        state_root: hash(0xf0),
        ordered_transactions,
        receipts,
        withdrawals: vec![],
        timestamp,
    }
}

fn internal_message(from: Hash32, to: Hash32, body: &[u8]) -> TvmInternalMessage {
    TvmInternalMessage {
        from,
        to,
        value: 0,
        body_boc: body.to_vec(),
        bounce: true,
        bounced: false,
    }
}

fn transfer_tx(seed: u8) -> SignedL2Transaction {
    SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: CHAIN_ID.to_owned(),
        from: Some(sha256_bytes(&[seed, 0x01])),
        nonce: u64::from(seed),
        valid_until_block: 10_000,
        gas_limit: 1_000 + u64::from(seed),
        max_gas_price: 1 + u128::from(seed),
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind: L2TransactionKind::Transfer {
            to: hash(0x20),
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 10,
        },
        public_key: None,
        signature: None,
    }
}

fn deposit_tx(seed: u8) -> SignedL2Transaction {
    SignedL2Transaction::system_deposit(
        CHAIN_ID,
        sha256_bytes(&[seed, 0x02]),
        L2_NATIVE_GAS_ASSET,
        sha256_bytes(&[seed, 0x03]),
        1_000 + u128::from(seed),
    )
}

fn seeded_index(seed: usize, len: usize) -> usize {
    assert!(len > 0);
    (seed * 17 + 11) % len
}

fn seeded_byte(seed: usize) -> u8 {
    ((seed * 31 + 7) as u8).max(1)
}

fn hash(byte: u8) -> Hash32 {
    Hash32::new([byte; 32])
}
