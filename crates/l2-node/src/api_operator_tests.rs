use super::*;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderValue;
use l2_core::crypto::sha256_bytes;
use l2_core::{canonical_batch_data_hash, DepositEvent, L2Block};

const ADMIN_TOKEN: &str = "test-admin-token";

fn test_state(admin_token: Option<&str>) -> AppState {
    AppState::test(admin_token)
}

fn auth_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("valid header"),
    );
    headers
}

fn deposit_event() -> DepositEvent {
    DepositEvent {
        deposit_id: sha256_bytes(b"deposit"),
        asset_id: 0,
        recipient: sha256_bytes(b"recipient"),
        amount: 100,
        l1_tx_hash: sha256_bytes(b"l1-tx"),
        l1_lt: 1,
    }
}

fn empty_block(height: u64) -> L2Block {
    L2Block::new(
        height,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"state"),
        vec![],
        vec![],
        vec![],
        canonical_batch_data_hash(&[], &[]),
        100,
    )
}

#[tokio::test]
async fn operator_metrics_requires_admin_and_reports_node_metrics() {
    let unauthorized = operator_metrics(State(test_state(None)), auth_headers(ADMIN_TOKEN))
        .await
        .unwrap_err();
    assert_eq!(unauthorized.status, StatusCode::FORBIDDEN);

    let state = test_state(Some(ADMIN_TOKEN));
    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit_event()),
    )
    .await
    .expect("deposit");
    produce_block_once(&state)
        .await
        .expect("produce")
        .expect("block");

    let metrics = operator_metrics(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("operator metrics");

    assert_eq!(metrics.0.node.block_production.produced, 1);
    assert_eq!(metrics.0.node.block_production.last_height, Some(0));
    assert_eq!(metrics.0.node.proposer.mode, "single_trusted");
    assert_eq!(metrics.0.node.proposer.proposal_observations, 1);
    assert_eq!(metrics.0.node.proposer.leader_lock_contention, 0);
    assert_eq!(metrics.0.node.latency.storage_save_block.operations, 1);
}

#[tokio::test]
async fn operator_metrics_reports_leader_lock_contention() {
    let state = test_state(Some(ADMIN_TOKEN));
    state
        .mempool
        .acquire_leader_lock("competing-producer")
        .await
        .expect("leader lock");

    let produced = produce_block_once(&state).await.expect("produce attempt");
    assert!(produced.is_none());

    let metrics = operator_metrics(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("operator metrics");
    assert_eq!(metrics.0.node.block_production.empty, 1);
    assert_eq!(metrics.0.node.proposer.leader_lock_contention, 1);
    assert_eq!(metrics.0.node.proposer.proposal_observations, 0);
}

#[tokio::test]
async fn operator_failures_reports_relayer_and_withdrawal_visibility() {
    let state = test_state(Some(ADMIN_TOKEN));
    state.storage.save_block(empty_block(0)).await.unwrap();
    let mut record = state.storage.get_batch_commit(1).await.unwrap().unwrap();
    record.status = crate::storage::BatchCommitStatus::Failed;
    record.attempts = 1;
    record.last_error = Some("batch data unavailable".to_owned());
    state.storage.save_batch_commit(record).await.unwrap();
    state
        .storage
        .save_batch_finalization(crate::storage::BatchFinalizationRecord {
            batch_no: 1,
            block_height: 0,
            status: crate::storage::BatchFinalizationStatus::Failed,
            attempts: 1,
            finalize_after_unix: 0,
            message_hash: None,
            message_hash_norm: None,
            last_error: Some("finalize signer failed".to_owned()),
        })
        .await
        .unwrap();

    let failures = operator_failures(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("operator failures");

    assert_eq!(failures.0.relayer_failed_batches.len(), 1);
    assert_eq!(
        failures.0.relayer_failed_batches[0].last_error.as_deref(),
        Some("batch data unavailable")
    );
    assert_eq!(failures.0.failed_finalizations.len(), 1);
    assert_eq!(
        failures.0.failed_finalizations[0].last_error.as_deref(),
        Some("finalize signer failed")
    );
    assert!(!failures.0.failed_withdrawals.indexed);
    assert_eq!(
        failures.0.failed_withdrawals.runbook,
        "docs/operator-runbooks.md#withdrawal-release-failures"
    );
}

#[tokio::test]
async fn operator_batch_finalizer_reports_status_groups() {
    let state = test_state(Some(ADMIN_TOKEN));
    state.storage.save_block(empty_block(0)).await.unwrap();
    let commit = state.storage.get_batch_commit(1).await.unwrap().unwrap();
    state
        .storage
        .save_batch_finalization(crate::storage::BatchFinalizationRecord::pending(
            &commit, 123,
        ))
        .await
        .unwrap();

    let visibility = operator_batch_finalizer(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("operator finalizer");

    assert_eq!(visibility.0.pending_finalization.len(), 1);
    assert_eq!(visibility.0.latest.as_ref().unwrap().batch_no, 1);
    assert!(visibility.0.latest_finalized.is_none());
}

#[tokio::test]
async fn operator_batch_relayer_reports_latest_commit() {
    let state = test_state(Some(ADMIN_TOKEN));
    state.storage.save_block(empty_block(0)).await.unwrap();

    let visibility = operator_batch_relayer(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("operator relayer");

    assert_eq!(visibility.0.pending.len(), 1);
    assert_eq!(visibility.0.latest.as_ref().unwrap().batch_no, 1);
    assert!(visibility.0.latest_confirmed.is_none());
}

#[tokio::test]
async fn operator_da_payload_visibility_requires_admin() {
    let error = operator_da_payload(
        State(test_state(None)),
        auth_headers(ADMIN_TOKEN),
        Path((0, Hash32::ZERO.to_hex())),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn operator_da_payload_visibility_reports_available_batch() {
    let state = test_state(Some(ADMIN_TOKEN));
    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit_event()),
    )
    .await
    .expect("deposit");
    let block = produce_block_once(&state)
        .await
        .expect("produce")
        .expect("block");

    let response = operator_da_payload(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Path((block.header.height, block.header.data_hash.to_hex())),
    )
    .await
    .expect("operator da")
    .0;

    assert_eq!(response.status, "available");
    assert_eq!(response.block_height, block.header.height);
    assert_eq!(response.data_hash, block.header.data_hash);
    assert_eq!(response.block_hash, Some(block.header.block_hash()));
    assert_eq!(
        response.payload_size,
        Some(l2_core::canonical_batch_data_bytes(&block.transactions, &block.receipts).len())
    );
    let expected_download_path = format!(
        "/v1/da/batch/{}/{}",
        block.header.height,
        block.header.data_hash.to_hex()
    );
    assert_eq!(
        response.download_path.as_deref(),
        Some(expected_download_path.as_str())
    );
    assert!(response.reason.is_none());
}

#[tokio::test]
async fn operator_da_payload_visibility_reports_missing_or_corrupt_batch() {
    let state = test_state(Some(ADMIN_TOKEN));
    let block = empty_block(0);
    state.storage.save_block(block.clone()).await.unwrap();

    let missing = operator_da_payload(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Path((block.header.height, block.header.data_hash.to_hex())),
    )
    .await
    .expect("missing da")
    .0;
    assert_eq!(missing.status, "missing");
    assert_eq!(missing.reason, Some("batch_data_unavailable"));

    let wrong_hash = operator_da_payload(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Path((block.header.height, sha256_bytes(b"wrong-data").to_hex())),
    )
    .await
    .expect("wrong hash")
    .0;
    assert_eq!(wrong_hash.status, "corrupt");
    assert_eq!(wrong_hash.reason, Some("data_hash_not_for_block"));

    state
        .storage
        .save_batch_payload(crate::storage::StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: vec![0],
            public_ref: None,
            public_uri: None,
        })
        .await
        .unwrap();

    let corrupt = operator_da_payload(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Path((block.header.height, block.header.data_hash.to_hex())),
    )
    .await
    .expect("corrupt da")
    .0;
    assert_eq!(corrupt.status, "corrupt");
    assert_eq!(corrupt.reason, Some("batch_data_hash_mismatch"));
}

#[tokio::test]
async fn operator_observer_replay_requires_authorization() {
    let state = test_state(None);
    let error = operator_observer_replay(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(crate::observer::ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![],
            store_checkpoint: false,
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn operator_observer_replay_validates_da_and_stores_checkpoint() {
    let state = test_state(Some(ADMIN_TOKEN));
    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit_event()),
    )
    .await
    .expect("deposit");
    let block = produce_block_once(&state)
        .await
        .expect("produce")
        .expect("block");
    let commitment = crate::signer::BatchCommitment::from_block(&block).expect("commitment");

    let report = operator_observer_replay(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(crate::observer::ObserverReplayRequest {
            trusted_checkpoint: None,
            commitments: vec![commitment],
            store_checkpoint: true,
        }),
    )
    .await
    .expect("observer replay");
    let checkpoint = operator_observer_checkpoint(State(state), auth_headers(ADMIN_TOKEN))
        .await
        .expect("checkpoint")
        .0
        .expect("stored checkpoint");

    assert_eq!(
        report.0.status,
        crate::observer::ObserverReplayStatus::Valid
    );
    assert_eq!(report.0.checked_batches, 1);
    assert_eq!(checkpoint.state_root, block.header.state_root);
}
