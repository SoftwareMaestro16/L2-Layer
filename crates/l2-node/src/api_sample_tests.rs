use super::*;
use crate::api::getter::ContractGetMethodRequest;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use l2_core::crypto::sha256_bytes;
use l2_core::{
    decode_contract_cell_boc_base64, l2_raw_address, l2_user_friendly_address,
    sample_counter_initial_state, Account, Hash32, L2Block, ENWALLET_V5R1_CODE_HASH,
    ENWALLET_V5R1_TESTNET_WALLET_ID, L2_NATIVE_GAS_ASSET,
};
use tonlib_core::cell::{BagOfCells, CellBuilder};

#[tokio::test]
async fn sample_counter_read_endpoint_returns_counter_state() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"sample-counter");
    let sample = sample_counter_initial_state(42);
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.code_hash = sample.code_hash;
        account.data_hash = sample.data_hash;
        account.storage_root = sample.storage_root;
        account.code_boc_base64 = Some(sample.code_boc_base64.clone());
        account.data_boc_base64 = Some(sample.data_boc_base64.clone());
    }

    let Json(response) = get_sample_counter(State(state), Path(l2_user_friendly_address(contract)))
        .await
        .expect("sample counter response");

    assert_eq!(response.contract, contract);
    assert_eq!(response.contract_raw_address, l2_raw_address(contract));
    assert_eq!(
        response.contract_friendly_address,
        l2_user_friendly_address(contract)
    );
    assert_eq!(response.counter, 42);
    assert_eq!(response.code_hash, sample.code_hash);
}

#[tokio::test]
async fn contract_get_method_reads_sample_counter() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"sample-counter");
    let sample = sample_counter_initial_state(5);
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.code_hash = sample.code_hash;
        account.data_hash = sample.data_hash;
        account.storage_root = sample.storage_root;
        account.code_boc_base64 = Some(sample.code_boc_base64);
        account.data_boc_base64 = Some(sample.data_boc_base64);
    }

    let Json(response) = get_contract_method(
        State(state),
        Path((
            l2_user_friendly_address(contract),
            "currentCounter".to_owned(),
        )),
    )
    .await
    .expect("get method response");

    assert_eq!(response.method, "currentCounter");
    assert_eq!(response.result["value"], "5");
    assert_eq!(response.source, "l2_state");
    assert!(response.read_only);
    assert_eq!(response.gas_used, 0);
    assert_eq!(response.vm_exit_code, 0);
}

#[tokio::test]
async fn contract_get_method_post_is_read_only_and_preserves_state_root() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"sample-counter");
    let sample = sample_counter_initial_state(8);
    let before_root = {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.code_hash = sample.code_hash;
        account.data_hash = sample.data_hash;
        account.storage_root = sample.storage_root;
        account.code_boc_base64 = Some(sample.code_boc_base64);
        account.data_boc_base64 = Some(sample.data_boc_base64);
        sequencer.state.root_hash()
    };

    let Json(response) = post_contract_get_method(
        State(state.clone()),
        Path(l2_user_friendly_address(contract)),
        Json(ContractGetMethodRequest {
            method: "currentCounter".to_owned(),
            method_id: None,
            stack_boc_base64: None,
            gas_limit: Some(25_000),
        }),
    )
    .await
    .expect("get method response");
    let after_root = state.sequencer.read().await.state.root_hash();

    assert_eq!(response.result["value"], "8");
    assert_eq!(response.gas_limit, 25_000);
    assert_eq!(response.state_root, before_root);
    assert_eq!(before_root, after_root);
}

#[tokio::test]
async fn contract_get_method_rejects_malformed_stack_payload() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"sample-counter");
    let sample = sample_counter_initial_state(8);
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.code_hash = sample.code_hash;
        account.data_hash = sample.data_hash;
        account.storage_root = sample.storage_root;
        account.code_boc_base64 = Some(sample.code_boc_base64);
        account.data_boc_base64 = Some(sample.data_boc_base64);
    }

    let error = post_contract_get_method(
        State(state),
        Path(l2_user_friendly_address(contract)),
        Json(ContractGetMethodRequest {
            method: "currentCounter".to_owned(),
            method_id: None,
            stack_boc_base64: Some("not-base64".to_owned()),
            gas_limit: None,
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "malformed_getter_stack_boc");
}

#[tokio::test]
async fn contract_get_method_rejects_over_limit_gas() {
    let mut state = AppState::test(Some("test-admin-token"));
    state.tvm_getter_max_gas_limit = 10;
    let contract = sha256_bytes(b"sample-counter");
    {
        let mut sequencer = state.sequencer.write().await;
        sequencer.state.account_mut(contract);
    }

    let error = post_contract_get_method(
        State(state),
        Path(l2_user_friendly_address(contract)),
        Json(ContractGetMethodRequest {
            method: "currentCounter".to_owned(),
            method_id: None,
            stack_boc_base64: None,
            gas_limit: Some(11),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "invalid_getter_gas_limit");
}

#[tokio::test]
async fn contract_get_method_reads_enwallet_v5_seqno() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"enwallet-v5");
    let data_boc_base64 = enwallet_data_boc(9);
    let data_hash = decode_contract_cell_boc_base64(&data_boc_base64, 16 * 1024)
        .unwrap()
        .cell_hash;
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.code_hash = ENWALLET_V5R1_CODE_HASH;
        account.data_hash = data_hash;
        account.storage_root = data_hash;
        account.data_boc_base64 = Some(data_boc_base64);
    }

    let Json(response) = get_contract_method(
        State(state),
        Path((l2_user_friendly_address(contract), "seqno".to_owned())),
    )
    .await
    .expect("get method response");

    assert_eq!(response.method, "seqno");
    assert_eq!(response.result["result"]["value"], "9");
    assert_eq!(response.source, "l2_state");
}

#[tokio::test]
async fn contract_getter_context_uses_stored_block_timestamp() {
    let state = AppState::test(Some("test-admin-token"));
    let block = L2Block::new(
        7,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state-root"),
        vec![],
        vec![],
        vec![],
        sha256_bytes(b"data-hash"),
        1_717_171,
    );
    state.storage.save_block(block).await.expect("save block");

    let account = Account {
        last_lt: 7,
        ..Account::default()
    };
    let context = getter::getter_execution_context(&state, &account)
        .await
        .expect("getter context");

    assert_eq!(context.block_height, 7);
    assert_eq!(context.block_time, 1_717_171);
    assert_eq!(context.gas_coin_asset, L2_NATIVE_GAS_ASSET);
    assert_eq!(context.max_internal_messages, 0);
}

#[tokio::test]
async fn sample_counter_read_endpoint_rejects_non_sample_account() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"ordinary-account");
    {
        let mut sequencer = state.sequencer.write().await;
        sequencer.state.account_mut(contract);
    }

    let error = get_sample_counter(State(state), Path(contract.to_hex()))
        .await
        .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "not a sample counter contract");
}

fn enwallet_data_boc(seqno: u32) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_bit(true)
        .unwrap()
        .store_u32(32, seqno)
        .unwrap()
        .store_u32(32, ENWALLET_V5R1_TESTNET_WALLET_ID)
        .unwrap()
        .store_bits(256, Hash32::new([0x33; 32]).as_bytes())
        .unwrap()
        .store_bit(false)
        .unwrap();
    let cell = builder.build().unwrap();
    let boc = BagOfCells::from_root(cell).serialize(false).unwrap();
    BASE64_STANDARD.encode(boc)
}
