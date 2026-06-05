use super::*;
use crate::faucet::EntFaucetRequest;
use axum::body::to_bytes;
use axum::http::header::AUTHORIZATION;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::IntoResponse;
use ed25519_dalek::{Signer, SigningKey};
use l2_core::crypto::{derive_account_id, sha256_bytes};
use l2_core::{
    canonical_batch_data_bytes, canonical_batch_data_hash, l2_raw_address,
    l2_user_friendly_address, AccountType, Hash32, L2Block, L2Event, L2TransactionKind, Receipt,
    WithdrawalLeaf, L2_NATIVE_GAS_ASSET, L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR,
    L2_TX_VERSION_V2,
};
use rand_core::OsRng;

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

fn peer(port: u16) -> ConnectInfo<std::net::SocketAddr> {
    ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], port)))
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

fn withdrawal_block() -> L2Block {
    let withdrawal = WithdrawalLeaf::new(
        sha256_bytes(b"withdrawal-tx"),
        0,
        100,
        sha256_bytes(b"withdrawal-sender"),
        "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR".to_owned(),
    );
    L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"withdrawal-state"),
        vec![],
        vec![],
        vec![withdrawal],
        canonical_batch_data_hash(&[], &[]),
        100,
    )
}

fn signed_tx(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    kind: L2TransactionKind,
) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit: 1_000,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
        kind,
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    tx
}

#[test]
fn admin_auth_is_fail_closed_when_disabled() {
    let auth = AdminAuth::new(None);
    let error = auth.authorize(&auth_headers(ADMIN_TOKEN)).unwrap_err();
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "admin api disabled");
}

#[test]
fn admin_auth_rejects_missing_malformed_and_wrong_tokens() {
    let auth = AdminAuth::new(Some(ADMIN_TOKEN.to_owned()));

    let missing = auth.authorize(&HeaderMap::new()).unwrap_err();
    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);

    let mut malformed_headers = HeaderMap::new();
    malformed_headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc"));
    let malformed = auth.authorize(&malformed_headers).unwrap_err();
    assert_eq!(malformed.status, StatusCode::UNAUTHORIZED);

    let wrong = auth.authorize(&auth_headers("wrong-token")).unwrap_err();
    assert_eq!(wrong.status, StatusCode::FORBIDDEN);
}

#[test]
fn admin_auth_accepts_correct_bearer_token() {
    let auth = AdminAuth::new(Some(ADMIN_TOKEN.to_owned()));
    assert!(auth.authorize(&auth_headers(ADMIN_TOKEN)).is_ok());
}

#[test]
fn deposit_event_validation_rejects_invalid_payload() {
    let mut deposit = deposit_event();
    deposit.deposit_id = Hash32::ZERO;
    let error = validate_deposit_event(&deposit).unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    let mut deposit = deposit_event();
    deposit.amount = 0;
    let error = validate_deposit_event(&deposit).unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);

    let mut deposit = deposit_event();
    deposit.l1_tx_hash = Hash32::ZERO;
    let error = validate_deposit_event(&deposit).unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn healthz_reports_process_alive() {
    let response = healthz().await;

    assert_eq!(response.0.status, "alive");
    assert_eq!(response.0.service, "entropis-l2-node");
}

#[tokio::test]
async fn readyz_reports_safe_component_statuses() {
    let state = test_state(Some(ADMIN_TOKEN));

    let (status, Json(report)) = readyz(State(state)).await;
    let rendered = serde_json::to_string(&report).expect("readiness json");

    assert_eq!(status, StatusCode::OK);
    assert_eq!(report.status, "ready");
    assert_eq!(report.components["db"].code, "ok");
    assert_eq!(report.components["redis"].code, "ok");
    assert_eq!(report.components["ton"].code, "ok");
    assert!(!rendered.contains(ADMIN_TOKEN));
    assert!(!rendered.contains("redis://"));
    assert!(!rendered.contains("postgresql://"));
    assert!(!rendered.contains("TONCENTER"));
}

#[test]
fn internal_errors_map_to_safe_public_messages() {
    let storage: ApiError = crate::storage::StorageError::Conflict {
        resource: "secret\ninjection",
    }
    .into();
    assert_eq!(storage.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(storage.message, "storage error");

    let da: ApiError = crate::da::DaError::Unavailable.into();
    assert_eq!(da.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(da.message, "data availability error");
}

#[tokio::test]
async fn admin_deposit_requires_authorization() {
    let state = test_state(None);
    let error = admin_deposit(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(deposit_event()),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_deposit_rejects_invalid_payload() {
    let state = test_state(Some(ADMIN_TOKEN));
    let mut deposit = deposit_event();
    deposit.recipient = Hash32::ZERO;

    let error = admin_deposit(State(state), auth_headers(ADMIN_TOKEN), Json(deposit))
        .await
        .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "reserved zero address");
}

#[tokio::test]
async fn admin_deposit_is_dev_mode_only() {
    let mut state = test_state(Some(ADMIN_TOKEN));
    state.dev_admin_deposits_enabled = false;

    let error = admin_deposit(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(deposit_event()),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "dev admin deposits disabled");
}

#[tokio::test]
async fn admin_deposit_accepts_authorized_valid_payload() {
    let state = test_state(Some(ADMIN_TOKEN));
    let deposit = deposit_event();
    let recipient = deposit.recipient;

    let status = admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit),
    )
    .await
    .expect("authorized deposit");
    assert_eq!(status, StatusCode::ACCEPTED);

    produce_block_once(&state)
        .await
        .expect("storage")
        .expect("deposit block");
    let sequencer = state.sequencer.read().await;
    assert_eq!(sequencer.state.account(recipient).unwrap().balance(0), 100);
}

#[tokio::test]
async fn admin_deposit_is_idempotent_through_storage() {
    let state = test_state(Some(ADMIN_TOKEN));
    let deposit = deposit_event();
    let recipient = deposit.recipient;

    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit.clone()),
    )
    .await
    .expect("first deposit");
    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit),
    )
    .await
    .expect("duplicate deposit");

    produce_block_once(&state)
        .await
        .expect("storage")
        .expect("deposit block");
    let sequencer = state.sequencer.read().await;
    assert_eq!(sequencer.state.account(recipient).unwrap().balance(0), 100);
}

#[tokio::test]
async fn batch_da_payload_is_served_by_height_and_data_hash() {
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
    let expected_payload = canonical_batch_data_bytes(&block.transactions, &block.receipts);

    let by_height = get_batch_da_payload(State(state.clone()), Path(block.header.height))
        .await
        .expect("batch da by height")
        .into_response();
    assert_eq!(by_height.status(), StatusCode::OK);
    assert_eq!(
        by_height
            .headers()
            .get(CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        by_height
            .headers()
            .get("x-entropis-data-hash")
            .unwrap()
            .to_str()
            .unwrap(),
        block.header.data_hash.to_hex()
    );
    let by_height_body = to_bytes(by_height.into_body(), usize::MAX)
        .await
        .expect("body");
    assert_eq!(by_height_body.as_ref(), expected_payload.as_slice());

    let by_hash = get_batch_da_payload_by_hash(
        State(state),
        Path((block.header.height, block.header.data_hash.to_hex())),
    )
    .await
    .expect("batch da by hash")
    .into_response();
    let by_hash_body = to_bytes(by_hash.into_body(), usize::MAX)
        .await
        .expect("body");
    assert_eq!(by_hash_body.as_ref(), expected_payload.as_slice());
}

#[tokio::test]
async fn batch_da_payload_hash_lookup_rejects_invalid_or_missing_hash() {
    let state = test_state(Some(ADMIN_TOKEN));

    let invalid = match get_batch_da_payload_by_hash(
        State(state.clone()),
        Path((0, "not-a-hash".to_owned())),
    )
    .await
    {
        Ok(_) => panic!("invalid hash unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(invalid.status, StatusCode::BAD_REQUEST);

    let missing = match get_batch_da_payload_by_hash(
        State(state),
        Path((0, sha256_bytes(b"missing-data").to_hex())),
    )
    .await
    {
        Ok(_) => panic!("missing hash unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn failed_da_write_does_not_commit_sequencer_state() {
    let state = test_state(Some(ADMIN_TOKEN));
    let deposit = deposit_event();
    let recipient = deposit.recipient;
    admin_deposit(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(deposit),
    )
    .await
    .expect("deposit");
    state
        .storage
        .save_batch_payload(crate::storage::StoredBatchPayload {
            block_height: 0,
            block_hash: sha256_bytes(b"old-block"),
            data_hash: sha256_bytes(b"old-data"),
            payload_bytes: vec![1, 2, 3],
            public_ref: None,
            public_uri: None,
        })
        .await
        .expect("conflicting old payload");

    let error = produce_block_once(&state).await.unwrap_err();

    assert_eq!(error.message, "data availability error");
    let sequencer = state.sequencer.read().await;
    assert!(sequencer.state.account(recipient).is_none());
    assert!(!sequencer.mempool.is_empty());
}

#[tokio::test]
async fn submit_tx_rejects_duplicate_before_block() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"recipient"),
            asset_id: 0,
            amount: 1,
        },
    );

    let first = submit_tx(State(state.clone()), peer(10001), Json(tx.clone()))
        .await
        .expect("first tx");
    assert_eq!(first.0.tx_hash, tx.tx_hash());

    let duplicate = submit_tx(State(state), peer(10001), Json(tx))
        .await
        .unwrap_err();
    assert_eq!(duplicate.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn submit_tx_rejects_malformed_system_deposit() {
    let state = test_state(Some(ADMIN_TOKEN));
    let forged_deposit = SignedL2Transaction::system_deposit(
        "entropis-testnet",
        sha256_bytes(b"forged"),
        0,
        sha256_bytes(b"recipient"),
        100,
    );

    let error = submit_tx(State(state), peer(10002), Json(forged_deposit))
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mempool_metrics_reports_rejections_and_queue_depth() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"recipient"),
            asset_id: 0,
            amount: 1,
        },
    );

    let _ = submit_tx(State(state.clone()), peer(10003), Json(tx.clone()))
        .await
        .expect("first tx");
    let duplicate = submit_tx(State(state.clone()), peer(10003), Json(tx))
        .await
        .unwrap_err();
    assert_eq!(duplicate.status, StatusCode::CONFLICT);

    let metrics = state.mempool.metrics().await.expect("metrics");
    assert_eq!(metrics.accepted, 1);
    assert_eq!(metrics.rejected.get("duplicate_tx"), Some(&1));
    assert_eq!(metrics.store.queued_global, 1);
}

#[tokio::test]
async fn submit_tx_enforces_per_ip_rate_limit_and_reports_reason() {
    let mut state = test_state(Some(ADMIN_TOKEN));
    state.mempool_ingress = MempoolIngressGuard::test(Duration::from_secs(60), 1, vec![]);
    let first_key = SigningKey::generate(&mut OsRng);
    let first_account = derive_account_id(&first_key.verifying_key().to_bytes());
    let second_key = SigningKey::generate(&mut OsRng);
    let second_account = derive_account_id(&second_key.verifying_key().to_bytes());

    let first = signed_tx(
        &first_key,
        first_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"first"),
            asset_id: 0,
            amount: 1,
        },
    );
    let second = signed_tx(
        &second_key,
        second_account,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"second"),
            asset_id: 0,
            amount: 1,
        },
    );

    let _ = submit_tx(State(state.clone()), peer(10004), Json(first))
        .await
        .expect("first tx");
    let limited = submit_tx(State(state.clone()), peer(10004), Json(second))
        .await
        .unwrap_err();
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.message, "ip_rate_limited");

    let metrics = state.mempool.metrics().await.expect("metrics");
    assert_eq!(metrics.rejected.get("ip_rate_limited"), Some(&1));
}

#[tokio::test]
async fn submit_tx_rejects_operator_banned_ip() {
    let mut state = test_state(Some(ADMIN_TOKEN));
    let banned_ip = "127.0.0.9".parse().unwrap();
    state.mempool_ingress = MempoolIngressGuard::test(Duration::from_secs(60), 10, vec![banned_ip]);
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"recipient"),
            asset_id: 0,
            amount: 1,
        },
    );

    let error = submit_tx(
        State(state.clone()),
        ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 9], 10005))),
        Json(tx),
    )
    .await
    .unwrap_err();
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "ip_banned");
    let metrics = state.mempool.metrics().await.expect("metrics");
    assert_eq!(metrics.rejected.get("ip_banned"), Some(&1));
}

#[tokio::test]
async fn tx_receipt_reports_pending_mempool_transaction() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"receipt-pending-recipient"),
            asset_id: 0,
            amount: 1,
        },
    );
    let tx_hash = submit_tx(State(state.clone()), peer(10006), Json(tx))
        .await
        .expect("submitted")
        .0
        .tx_hash;

    let response = get_receipt(State(state), Path(tx_hash.to_hex()))
        .await
        .expect("pending receipt")
        .0;

    assert_eq!(response.status, "pending");
    assert!(response.transaction.is_some());
    assert!(response.receipt.is_none());
    assert!(response.block.is_none());
    assert!(response.finality.is_none());
}

#[tokio::test]
async fn tx_receipt_reports_rejected_reason_and_gas() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"receipt-rejected-recipient"),
            asset_id: 0,
            amount: 1,
        },
    );
    let receipt = Receipt::rejected_with_gas(tx.tx_hash(), "insufficient_balance", 7);
    let block = L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"receipt-rejected-state"),
        vec![tx.clone()],
        vec![receipt],
        vec![],
        canonical_batch_data_hash(
            std::slice::from_ref(&tx),
            &[Receipt::rejected_with_gas(
                tx.tx_hash(),
                "insufficient_balance",
                7,
            )],
        ),
        10,
    );
    state.storage.save_block(block).await.unwrap();

    let response = get_tx_receipt(State(state), Path(tx.tx_hash().to_hex()))
        .await
        .expect("rejected receipt")
        .0;

    assert_eq!(response.status, "rejected");
    let receipt = response.receipt.expect("receipt detail");
    assert_eq!(receipt.status, "rejected");
    assert_eq!(receipt.gas_charged, 7);
    assert_eq!(receipt.reason.as_deref(), Some("insufficient_balance"));
    assert!(receipt.events.is_empty());
    assert!(receipt.contract_logs.is_empty());
}

#[tokio::test]
async fn tx_receipt_reports_committed_events_and_logs() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let contract = sha256_bytes(b"receipt-event-contract");
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::CallContract {
            contract,
            body_boc_base64: "te6ccgEBAQEAAgAAAA==".to_owned(),
        },
    );
    let event = L2Event::ContractCalled {
        contract,
        caller: account_id,
        body_hash: sha256_bytes(b"body"),
    };
    let receipt = Receipt::applied(tx.tx_hash(), 5, None).with_events(vec![event.clone()]);
    let data_hash =
        canonical_batch_data_hash(std::slice::from_ref(&tx), std::slice::from_ref(&receipt));
    let block = L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"receipt-event-state"),
        vec![tx.clone()],
        vec![receipt],
        vec![],
        data_hash,
        10,
    );
    state.storage.save_block(block).await.unwrap();

    let response = get_receipt(State(state), Path(tx.tx_hash().to_hex()))
        .await
        .expect("event receipt")
        .0;

    let receipt = response.receipt.expect("receipt detail");
    assert_eq!(receipt.events, vec![event]);
    assert_eq!(receipt.contract_logs.len(), 1);
    assert_eq!(receipt.contract_logs[0].contract, Some(contract));
    assert_eq!(receipt.contract_logs[0].message, "contract_called");
}

#[tokio::test]
async fn tx_receipt_and_block_finality_report_finalized_batch() {
    let state = test_state(Some(ADMIN_TOKEN));
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"receipt-finalized-recipient"),
            asset_id: 0,
            amount: 1,
        },
    );
    let receipt = Receipt::applied(tx.tx_hash(), 3, None);
    let data_hash =
        canonical_batch_data_hash(std::slice::from_ref(&tx), std::slice::from_ref(&receipt));
    let block = L2Block::new(
        0,
        Hash32::ZERO,
        Hash32::ZERO,
        sha256_bytes(b"receipt-finalized-state"),
        vec![tx.clone()],
        vec![receipt],
        vec![],
        data_hash,
        11,
    );
    state.storage.save_block(block).await.unwrap();
    let mut commit = state.storage.get_batch_commit(1).await.unwrap().unwrap();
    commit.status = crate::storage::BatchCommitStatus::Confirmed;
    commit.attempts = 1;
    commit.message_hash_norm = Some(sha256_bytes(b"commit-norm"));
    state
        .storage
        .save_batch_commit(commit.clone())
        .await
        .unwrap();
    let mut finalization = crate::storage::BatchFinalizationRecord::pending(&commit, 12);
    finalization.status = crate::storage::BatchFinalizationStatus::Finalized;
    finalization.attempts = 1;
    finalization.message_hash_norm = Some(sha256_bytes(b"finalize-norm"));
    state
        .storage
        .save_batch_finalization(finalization)
        .await
        .unwrap();

    let receipt_response = get_receipt(State(state.clone()), Path(tx.tx_hash().to_hex()))
        .await
        .expect("finalized receipt")
        .0;
    assert_eq!(receipt_response.status, "finalized");
    let finality = receipt_response.finality.expect("finality");
    assert!(finality.committed);
    assert!(finality.finalized);
    assert_eq!(finality.commit.unwrap().status, "confirmed");
    assert_eq!(finality.finalization.unwrap().status, "finalized");

    let block_finality = get_block_finality(State(state), Path(0))
        .await
        .expect("block finality")
        .0;
    assert_eq!(block_finality.batch_no, 1);
    assert!(block_finality.committed);
    assert!(block_finality.finalized);
}

#[tokio::test]
async fn withdrawal_proof_is_hidden_until_batch_finalized() {
    let state = test_state(Some(ADMIN_TOKEN));
    let block = withdrawal_block();
    let withdrawal_id = block.withdrawals[0].withdrawal_id;
    state.storage.save_block(block).await.unwrap();

    let error = get_withdrawal_proof(State(state), Path(withdrawal_id.to_hex()))
        .await
        .err()
        .expect("proof is gated");

    assert_eq!(error.status, StatusCode::CONFLICT);
    assert_eq!(error.message, "withdrawal batch not finalized");
}

#[tokio::test]
async fn withdrawal_proof_is_served_after_batch_finalization() {
    let state = test_state(Some(ADMIN_TOKEN));
    let block = withdrawal_block();
    let withdrawal_id = block.withdrawals[0].withdrawal_id;
    state.storage.save_block(block).await.unwrap();
    let commit = state.storage.get_batch_commit(1).await.unwrap().unwrap();
    let mut finalization = crate::storage::BatchFinalizationRecord::pending(&commit, 0);
    finalization.status = crate::storage::BatchFinalizationStatus::Finalized;
    finalization.attempts = 1;
    finalization.message_hash = Some(sha256_bytes(b"finalize-message"));
    finalization.message_hash_norm = Some(sha256_bytes(b"finalize-message-norm"));
    state
        .storage
        .save_batch_finalization(finalization)
        .await
        .unwrap();

    get_withdrawal_proof(State(state), Path(withdrawal_id.to_hex()))
        .await
        .expect("finalized withdrawal proof");
}

#[tokio::test]
async fn admin_ent_faucet_requires_authorization() {
    let state = test_state(None);
    let account_id = sha256_bytes(b"account");

    let error = admin_ent_faucet(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetRequest {
            account_id: account_id.to_hex(),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_ent_faucet_rejects_invalid_account() {
    let state = test_state(Some(ADMIN_TOKEN));

    let error = admin_ent_faucet(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetRequest {
            account_id: "not-a-hash".to_owned(),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_ent_faucet_rejects_reserved_zero_address() {
    let state = test_state(Some(ADMIN_TOKEN));

    let error = admin_ent_faucet(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetRequest {
            account_id: l2_raw_address(Hash32::ZERO),
        }),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert_eq!(error.message, "reserved zero address");
}

#[tokio::test]
async fn admin_ent_faucet_is_idempotent_and_credits_ent_base_units() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"faucet-account");
    let request = EntFaucetRequest {
        account_id: l2_raw_address(account_id),
    };

    let first = admin_ent_faucet(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(request.clone()),
    )
    .await
    .expect("first faucet grant");
    assert!(first.0.granted);
    assert_eq!(first.0.amount_base_units, 1_000_000_000_000);

    let duplicate = admin_ent_faucet(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetRequest {
            account_id: l2_user_friendly_address(account_id),
        }),
    )
    .await
    .expect("duplicate faucet grant");
    assert!(!duplicate.0.granted);
    assert_eq!(duplicate.0.deposit_id, first.0.deposit_id);

    produce_block_once(&state)
        .await
        .expect("storage")
        .expect("faucet block");
    let sequencer = state.sequencer.read().await;
    assert_eq!(
        sequencer.state.account(account_id).unwrap().balance(0),
        1_000_000_000_000
    );
    drop(sequencer);

    get_account(State(state), Path(l2_user_friendly_address(account_id)))
        .await
        .expect("friendly account path");
}

#[tokio::test]
async fn account_metadata_reports_lifecycle_without_private_fields() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"metadata-account");
    {
        let mut sequencer = state.sequencer.write().await;
        let account = sequencer.state.account_mut(account_id);
        account.account_type = AccountType::Operator;
        account.flags.disabled = true;
        account.active_public_key = Some(Hash32::new([9; 32]));
        account.credit(0, 100);
        account.last_lt = 5;
    }

    let metadata = get_account_metadata(State(state.clone()), Path(l2_raw_address(account_id)))
        .await
        .expect("metadata")
        .0;

    assert_eq!(metadata.account_id, account_id);
    assert_eq!(metadata.status, "disabled");
    assert_eq!(metadata.account_type, AccountType::Operator);
    assert!(metadata.flags.disabled);
    assert!(metadata.active_public_key_set);
    assert_eq!(metadata.active_public_key, Some(Hash32::new([9; 32])));
    assert_eq!(metadata.has_code, false);
    assert_eq!(metadata.has_data, false);
    assert_eq!(metadata.last_lt, 5);

    let zero = get_account_metadata(State(state), Path(l2_user_friendly_address(Hash32::ZERO)))
        .await
        .expect("zero metadata")
        .0;
    assert_eq!(zero.status, "reserved");
    assert_eq!(zero.account_type, AccountType::System);
    assert!(zero.flags.system_only);
    assert!(zero.flags.disabled);
}
