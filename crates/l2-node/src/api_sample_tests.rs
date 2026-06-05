use super::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use l2_core::crypto::sha256_bytes;
use l2_core::{
    decode_contract_cell_boc_base64, l2_raw_address, l2_user_friendly_address,
    sample_counter_initial_state, Hash32, ENWALLET_V5R1_CODE_HASH, ENWALLET_V5R1_TESTNET_WALLET_ID,
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
