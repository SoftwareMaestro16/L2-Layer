use super::*;
use l2_core::{crypto::sha256_bytes, l2_user_friendly_address, sample_counter_initial_state};

#[tokio::test]
async fn contract_state_endpoint_reads_live_contract_cells() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"live-contract-state");
    let sample = sample_counter_initial_state(11);
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(contract);
        account.mark_contract_account();
        account.code_hash = sample.code_hash;
        account.data_hash = sample.data_hash;
        account.storage_root = sample.storage_root;
        account.code_boc_base64 = Some(sample.code_boc_base64.clone());
        account.data_boc_base64 = Some(sample.data_boc_base64.clone());
        account.last_lt = 7;
    }

    let Json(response) = get_contract_state(State(state), Path(l2_user_friendly_address(contract)))
        .await
        .expect("contract state response");

    assert_eq!(response.contract, contract);
    assert_eq!(response.source, "l2_state");
    assert_eq!(response.code.code_hash, sample.code_hash);
    assert_eq!(response.code.code_boc_base64, sample.code_boc_base64);
    assert_eq!(response.data.data_hash, sample.data_hash);
    assert_eq!(response.data.storage_root, sample.storage_root);
    assert_eq!(response.data.data_boc_base64, sample.data_boc_base64);
    assert_eq!(response.last_block_height, 7);
}

#[tokio::test]
async fn contract_state_endpoint_reads_persisted_registry_after_restart() {
    let state = AppState::test(Some("test-admin-token"));
    let contract = sha256_bytes(b"persisted-contract-state");
    let sample = sample_counter_initial_state(21);
    let mut account = l2_core::Account::default();
    account.mark_contract_account();
    account.code_hash = sample.code_hash;
    account.data_hash = sample.data_hash;
    account.storage_root = sample.storage_root;
    account.code_boc_base64 = Some(sample.code_boc_base64.clone());
    account.data_boc_base64 = Some(sample.data_boc_base64.clone());
    account.last_lt = 9;
    let record = crate::storage::StoredContractState::from_account(contract, &account, 9)
        .expect("valid contract state")
        .expect("contract record");
    state
        .storage
        .save_contract_state(record)
        .await
        .expect("save contract state");

    let Json(response) = get_contract_state(State(state), Path(l2_user_friendly_address(contract)))
        .await
        .expect("contract state response");

    assert_eq!(response.contract, contract);
    assert_eq!(response.source, "storage_registry");
    assert_eq!(response.account, account);
    assert_eq!(response.code.code_hash, sample.code_hash);
    assert_eq!(response.data.data_hash, sample.data_hash);
    assert_eq!(response.last_block_height, 9);
}
