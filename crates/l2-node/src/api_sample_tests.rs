use super::*;
use l2_core::crypto::sha256_bytes;
use l2_core::{l2_raw_address, l2_user_friendly_address, sample_counter_initial_state};

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
