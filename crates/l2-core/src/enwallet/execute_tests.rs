use super::*;
use crate::crypto::{derive_account_id, Hash32};
use crate::executor::{DeterministicExecutor, ExecutionConfig, TvmAdapterMode};
use crate::state::State;
use crate::tvm::{
    boc_single_root_hash, decode_contract_cell_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES,
};
use crate::types::{
    L2Event, L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use tonlib_core::cell::{BagOfCells, CellBuilder};
const EXTERNAL_SIGNED_REQUEST: u32 = 0x7369_676e;

#[test]
fn enwallet_deploy_call_and_read_state_uses_real_bocs() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let owner = derive_account_id(&signing_key.verifying_key().to_bytes());
    let wallet = crate::crypto::sha256_bytes(b"enwallet-v5-l2-e2e");
    let code_boc_base64 = enwallet_code_boc_base64();
    let data_boc_base64 = enwallet_data_boc_base64(0, &signing_key);

    let executor = DeterministicExecutor;
    let mut state = State::default();
    assert!(state.account_mut(owner).credit(L2_NATIVE_GAS_ASSET, 1_000));

    let deploy = executor.apply(
        &mut state,
        &tx(
            owner,
            0,
            L2TransactionKind::DeployContract {
                contract: wallet,
                code_boc_base64,
                data_boc_base64,
            },
        ),
        &ExecutionConfig::default(),
    );
    assert_eq!(deploy.receipt.status, ReceiptStatus::Applied);
    assert_eq!(
        deploy.receipt.events,
        vec![L2Event::ContractDeployed {
            contract: wallet,
            deployer: owner,
            code_hash: ENWALLET_V5R1_CODE_HASH,
            data_hash: state.account(wallet).unwrap().data_hash,
        }]
    );

    let call_config = ExecutionConfig {
        block_time: 100,
        block_height: 2,
        tvm_adapter_mode: TvmAdapterMode::Prototype,
        ..ExecutionConfig::default()
    };
    let before_root = state.root_hash();
    let call = executor.apply(
        &mut state,
        &tx(
            owner,
            1,
            L2TransactionKind::CallContract {
                contract: wallet,
                body_boc_base64: signed_body(&signing_key, 0, 200, false, false),
            },
        ),
        &call_config,
    );

    let wallet_state = read_enwallet_v5_state(state.account(wallet).unwrap()).expect("wallet");
    assert_eq!(call.receipt.status, ReceiptStatus::Applied);
    assert_eq!(wallet_state.seqno, 1);
    assert_eq!(wallet_state.wallet_id, ENWALLET_V5R1_TESTNET_WALLET_ID);
    assert_eq!(
        wallet_state.public_key,
        Hash32::new(signing_key.verifying_key().to_bytes())
    );
    assert_ne!(before_root, state.root_hash());
    assert_eq!(state.account(owner).unwrap().nonce, 2);
}

#[test]
fn enwallet_signed_request_failures_do_not_mutate_wallet_state() {
    let signing_key = SigningKey::from_bytes(&[9; 32]);
    let wrong_key = SigningKey::from_bytes(&[10; 32]);

    for (body, reason) in [
        (
            signed_body(&signing_key, 7, 200, false, false),
            "enwallet_invalid_seqno",
        ),
        (
            signed_body_with_wallet_id(&signing_key, 0, 0x1111_2222, 200, false, false),
            "enwallet_invalid_wallet_id",
        ),
        (
            signed_body(&wrong_key, 0, 200, false, false),
            "enwallet_invalid_signature",
        ),
        (
            signed_body(&signing_key, 0, 100, false, false),
            "enwallet_expired",
        ),
        (
            signed_body(&signing_key, 0, 200, true, false),
            "enwallet_c5_actions_unsupported",
        ),
        (
            signed_body(&signing_key, 0, 200, false, true),
            "enwallet_extra_actions_unsupported",
        ),
        (empty_body_boc_base64(), "enwallet_malformed_body"),
    ] {
        let (outcome, seqno) = run_single_call(&signing_key, body);
        assert_eq!(outcome.receipt.status, ReceiptStatus::Rejected);
        assert_eq!(outcome.receipt.reason.as_deref(), Some(reason));
        assert_eq!(seqno, 0);
    }
}

fn run_single_call(signing_key: &SigningKey, body: String) -> (crate::ExecutionOutcome, u32) {
    let owner = derive_account_id(&signing_key.verifying_key().to_bytes());
    let wallet = crate::crypto::sha256_bytes(b"enwallet-negative");
    let mut state = State::default();
    assert!(state.account_mut(owner).credit(L2_NATIVE_GAS_ASSET, 1_000));
    let account = state.account_mut(wallet);
    account.mark_contract_account();
    account.code_hash = ENWALLET_V5R1_CODE_HASH;
    account.code_boc_base64 = Some(enwallet_code_boc_base64());
    let data_boc_base64 = enwallet_data_boc_base64(0, signing_key);
    let data_hash = cell_hash(&data_boc_base64);
    account.data_hash = data_hash;
    account.storage_root = data_hash;
    account.data_boc_base64 = Some(data_boc_base64);

    let outcome = DeterministicExecutor.apply(
        &mut state,
        &tx(
            owner,
            0,
            L2TransactionKind::CallContract {
                contract: wallet,
                body_boc_base64: body,
            },
        ),
        &ExecutionConfig {
            block_time: 100,
            tvm_adapter_mode: TvmAdapterMode::Prototype,
            ..ExecutionConfig::default()
        },
    );
    let seqno = read_enwallet_v5_state(state.account(wallet).unwrap())
        .expect("wallet state")
        .seqno;
    (outcome, seqno)
}

fn tx(from: Hash32, nonce: u64, kind: L2TransactionKind) -> SignedL2Transaction {
    SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit: 50,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind,
        public_key: None,
        signature: None,
    }
}

fn signed_body(
    signing_key: &SigningKey,
    seqno: u32,
    valid_until: u32,
    has_out_actions: bool,
    has_extra_actions: bool,
) -> String {
    signed_body_with_wallet_id(
        signing_key,
        seqno,
        ENWALLET_V5R1_TESTNET_WALLET_ID,
        valid_until,
        has_out_actions,
        has_extra_actions,
    )
}

fn signed_body_with_wallet_id(
    signing_key: &SigningKey,
    seqno: u32,
    wallet_id: u32,
    valid_until: u32,
    has_out_actions: bool,
    has_extra_actions: bool,
) -> String {
    let unsigned = request_cell(
        seqno,
        wallet_id,
        valid_until,
        has_out_actions,
        has_extra_actions,
        None,
    );
    let unsigned_hash = cell_hash(&cell_to_base64(unsigned));
    let signature = signing_key.sign(unsigned_hash.as_bytes()).to_bytes();
    let signed = request_cell(
        seqno,
        wallet_id,
        valid_until,
        has_out_actions,
        has_extra_actions,
        Some(&signature),
    );
    cell_to_base64(signed)
}

fn request_cell(
    seqno: u32,
    wallet_id: u32,
    valid_until: u32,
    has_out_actions: bool,
    has_extra_actions: bool,
    signature: Option<&[u8; 64]>,
) -> tonlib_core::cell::Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, EXTERNAL_SIGNED_REQUEST)
        .unwrap()
        .store_u32(32, wallet_id)
        .unwrap()
        .store_u32(32, valid_until)
        .unwrap()
        .store_u32(32, seqno)
        .unwrap()
        .store_bit(has_out_actions)
        .unwrap()
        .store_bit(has_extra_actions)
        .unwrap();
    if let Some(signature) = signature {
        builder.store_bits(512, signature).unwrap();
    }
    builder.build().unwrap()
}

fn enwallet_data_boc_base64(seqno: u32, signing_key: &SigningKey) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_bit(true)
        .unwrap()
        .store_u32(32, seqno)
        .unwrap()
        .store_u32(32, ENWALLET_V5R1_TESTNET_WALLET_ID)
        .unwrap()
        .store_bits(256, &signing_key.verifying_key().to_bytes())
        .unwrap()
        .store_bit(false)
        .unwrap();
    cell_to_base64(builder.build().unwrap())
}

fn enwallet_code_boc_base64() -> String {
    let source = include_str!("../../../../sdk/src/generated/EnWalletV5.gen.ts");
    let marker = "static CodeCell = c.Cell.fromBase64('";
    let start = source.find(marker).expect("code marker") + marker.len();
    let rest = &source[start..];
    let end = rest.find("');").expect("code end");
    let code = rest[..end].to_owned();
    assert_eq!(
        decode_contract_cell_boc_base64(&code, DEFAULT_MAX_TVM_BOC_BYTES)
            .unwrap()
            .cell_hash,
        ENWALLET_V5R1_CODE_HASH
    );
    code
}

fn empty_body_boc_base64() -> String {
    cell_to_base64(CellBuilder::new().build().unwrap())
}

fn cell_hash(value: &str) -> Hash32 {
    let boc = BASE64_STANDARD.decode(value.as_bytes()).unwrap();
    boc_single_root_hash(&boc).unwrap()
}

fn cell_to_base64(cell: tonlib_core::cell::Cell) -> String {
    let boc = BagOfCells::from_root(cell).serialize(false).unwrap();
    BASE64_STANDARD.encode(boc)
}
