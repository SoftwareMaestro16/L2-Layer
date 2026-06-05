use super::*;
use crate::crypto::{derive_account_id, sha256_bytes};
use crate::tvm::{
    TvmAdapterError, TvmExecutionInput, TvmExecutionOutput, TvmExecutionStatus, TvmInternalMessage,
};
use crate::types::{
    L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
use std::sync::{Arc, Mutex};
use tonlib_core::cell::{BagOfCells, CellBuilder};

#[derive(Clone)]
struct RoutingAdapter {
    calls: Arc<Mutex<Vec<(Hash32, Hash32)>>>,
    emit_contract: Hash32,
    emit_caller: Hash32,
    emitted: Vec<TvmInternalMessage>,
}

impl TvmExecutionAdapter for RoutingAdapter {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        self.calls
            .lock()
            .expect("calls lock")
            .push((input.caller, input.contract));
        Ok(TvmExecutionOutput {
            status: TvmExecutionStatus::Applied,
            state_delta: None,
            emitted_internal_messages: if input.contract == self.emit_contract
                && input.caller == self.emit_caller
            {
                self.emitted.clone()
            } else {
                vec![]
            },
            gas_used: 10,
        })
    }
}

fn account(seed: &[u8]) -> Hash32 {
    sha256_bytes(seed)
}

fn valid_boc() -> Vec<u8> {
    let cell = CellBuilder::new().build().expect("empty cell");
    BagOfCells::from_root(cell)
        .serialize(false)
        .expect("serialize boc")
}

fn valid_boc_base64() -> String {
    BASE64_STANDARD.encode(valid_boc())
}

fn install_contract(sequencer: &mut Sequencer, contract: Hash32) {
    let code_boc_base64 = valid_boc_base64();
    let data_boc_base64 = valid_boc_base64();
    let code_hash = crate::boc_single_root_hash(
        &BASE64_STANDARD
            .decode(code_boc_base64.as_bytes())
            .expect("code b64"),
    )
    .expect("code hash");
    let data_hash = crate::boc_single_root_hash(
        &BASE64_STANDARD
            .decode(data_boc_base64.as_bytes())
            .expect("data b64"),
    )
    .expect("data hash");
    let account = sequencer.state.account_mut(contract);
    account.mark_contract_account();
    account.code_hash = code_hash;
    account.data_hash = data_hash;
    account.storage_root = data_hash;
    account.code_boc_base64 = Some(code_boc_base64);
    account.data_boc_base64 = Some(data_boc_base64);
}

fn signed_call(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    contract: Hash32,
) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "ton-l2-devnet".to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit: 1_000,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind: L2TransactionKind::CallContract {
            contract,
            body_boc_base64: valid_boc_base64(),
        },
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    tx.signature = Some(hex::encode(
        signing_key.sign(&tx.signing_payload()).to_bytes(),
    ));
    tx
}

fn funded_user(sequencer: &mut Sequencer, amount: u128) -> (SigningKey, Hash32) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"deposit"),
        asset_id: L2_NATIVE_GAS_ASSET,
        recipient: account_id,
        amount,
        l1_tx_hash: sha256_bytes(b"l1"),
        l1_lt: 1,
    }]);
    (signing_key, account_id)
}

fn internal_message(from: Hash32, to: Hash32) -> TvmInternalMessage {
    TvmInternalMessage {
        from,
        to,
        value: 0,
        body_boc: valid_boc(),
        bounce: true,
        bounced: false,
    }
}

#[test]
fn contract_schedules_internal_message_for_next_block() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let (key, user) = funded_user(&mut sequencer, 10_000);
    let contract_a = account(b"contract-a");
    let contract_b = account(b"contract-b");
    install_contract(&mut sequencer, contract_a);
    install_contract(&mut sequencer, contract_b);
    sequencer.produce_block(1).expect("deposit block");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let adapter = RoutingAdapter {
        calls: Arc::clone(&calls),
        emit_contract: contract_a,
        emit_caller: user,
        emitted: vec![internal_message(contract_a, contract_b)],
    };

    sequencer.submit_tx(signed_call(&key, user, 0, contract_a));
    let block = sequencer
        .produce_block_with_test_tvm_adapter(2, &adapter)
        .expect("call block");

    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(sequencer.pending_internal_message_count(), 1);
    assert_eq!(
        calls.lock().expect("calls lock").as_slice(),
        &[(user, contract_a)]
    );

    let internal_block = sequencer
        .produce_block_with_test_tvm_adapter(3, &adapter)
        .expect("internal block");

    assert_eq!(internal_block.transactions.len(), 1);
    assert_eq!(internal_block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(sequencer.pending_internal_message_count(), 0);
    match &internal_block.transactions[0].kind {
        L2TransactionKind::InternalMessage {
            from,
            to,
            bounce,
            bounced,
            ..
        } => {
            assert_eq!((*from, *to), (contract_a, contract_b));
            assert!(*bounce);
            assert!(!*bounced);
        }
        other => panic!("expected internal message tx, got {other:?}"),
    }
    assert_eq!(
        calls.lock().expect("calls lock").as_slice(),
        &[(user, contract_a), (contract_a, contract_b)]
    );
}

#[test]
fn internal_queue_replay_is_deterministic() {
    fn run() -> (Hash32, Hash32) {
        let mut sequencer = Sequencer::new(SequencerConfig::default());
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let user = derive_account_id(&key.verifying_key().to_bytes());
        sequencer.ingest_deposits(vec![DepositEvent {
            deposit_id: sha256_bytes(b"deposit"),
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: user,
            amount: 10_000,
            l1_tx_hash: sha256_bytes(b"l1"),
            l1_lt: 1,
        }]);
        let contract_a = account(b"contract-a");
        let contract_b = account(b"contract-b");
        install_contract(&mut sequencer, contract_a);
        install_contract(&mut sequencer, contract_b);
        sequencer.produce_block(1).expect("deposit block");
        let adapter = RoutingAdapter {
            calls: Arc::new(Mutex::new(Vec::new())),
            emit_contract: contract_a,
            emit_caller: user,
            emitted: vec![internal_message(contract_a, contract_b)],
        };
        sequencer.submit_tx(signed_call(&key, user, 0, contract_a));
        let first = sequencer
            .produce_block_with_test_tvm_adapter(2, &adapter)
            .expect("call block");
        let second = sequencer
            .produce_block_with_test_tvm_adapter(3, &adapter)
            .expect("internal block");
        (first.header.block_hash(), second.header.block_hash())
    }

    assert_eq!(run(), run());
}

#[test]
fn message_explosion_is_rejected_when_queue_capacity_would_overflow() {
    let mut sequencer = Sequencer::new(SequencerConfig {
        max_internal_queue_len: 1,
        ..SequencerConfig::default()
    });
    let (key, user) = funded_user(&mut sequencer, 10_000);
    let contract_a = account(b"contract-a");
    install_contract(&mut sequencer, contract_a);
    sequencer.produce_block(1).expect("deposit block");
    let adapter = RoutingAdapter {
        calls: Arc::new(Mutex::new(Vec::new())),
        emit_contract: contract_a,
        emit_caller: user,
        emitted: vec![
            internal_message(contract_a, account(b"contract-b")),
            internal_message(contract_a, account(b"contract-c")),
        ],
    };

    sequencer.submit_tx(signed_call(&key, user, 0, contract_a));
    let block = sequencer
        .produce_block_with_test_tvm_adapter(2, &adapter)
        .expect("call block");

    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("internal_queue_full")
    );
    assert_eq!(sequencer.pending_internal_message_count(), 0);
    assert_eq!(sequencer.state.account(user).unwrap().nonce, 0);
}

#[test]
fn failed_internal_delivery_schedules_single_bounce() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let (key, user) = funded_user(&mut sequencer, 10_000);
    let contract_a = account(b"contract-a");
    let missing_b = account(b"missing-b");
    install_contract(&mut sequencer, contract_a);
    sequencer.produce_block(1).expect("deposit block");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let adapter = RoutingAdapter {
        calls: Arc::clone(&calls),
        emit_contract: contract_a,
        emit_caller: user,
        emitted: vec![internal_message(contract_a, missing_b)],
    };

    sequencer.submit_tx(signed_call(&key, user, 0, contract_a));
    sequencer
        .produce_block_with_test_tvm_adapter(2, &adapter)
        .expect("call block");
    let failed_delivery = sequencer
        .produce_block_with_test_tvm_adapter(3, &adapter)
        .expect("failed internal block");

    assert_eq!(failed_delivery.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        failed_delivery.receipts[0].reason.as_deref(),
        Some("unknown_contract")
    );
    assert_eq!(sequencer.pending_internal_message_count(), 1);

    let bounce = sequencer
        .produce_block_with_test_tvm_adapter(4, &adapter)
        .expect("bounce block");

    assert_eq!(bounce.receipts[0].status, ReceiptStatus::Applied);
    match &bounce.transactions[0].kind {
        L2TransactionKind::InternalMessage {
            from,
            to,
            bounce,
            bounced,
            ..
        } => {
            assert_eq!((*from, *to), (missing_b, contract_a));
            assert!(!*bounce);
            assert!(*bounced);
        }
        other => panic!("expected bounce internal message tx, got {other:?}"),
    }
    assert_eq!(
        calls.lock().expect("calls lock").as_slice(),
        &[(user, contract_a), (missing_b, contract_a)]
    );
}
