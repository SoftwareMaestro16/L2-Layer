use super::*;
use axum::http::HeaderValue;
use ed25519_dalek::{Signer, SigningKey};
use l2_core::crypto::derive_account_id;
use l2_core::L2TransactionKind;
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

fn signed_tx(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    kind: L2TransactionKind,
) -> SignedL2Transaction {
    let public_key = signing_key.verifying_key().to_bytes();
    let mut tx = SignedL2Transaction {
        chain_id: "entropis-testnet".to_owned(),
        from: Some(from),
        nonce,
        gas_limit: 1_000,
        max_gas_price: 1,
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

    let first = submit_tx(State(state.clone()), Json(tx.clone()))
        .await
        .expect("first tx");
    assert_eq!(first.0.tx_hash, tx.tx_hash());

    let duplicate = submit_tx(State(state), Json(tx)).await.unwrap_err();
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

    let error = submit_tx(State(state), Json(forged_deposit))
        .await
        .unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
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
async fn admin_ent_faucet_is_idempotent_and_credits_ent_base_units() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"faucet-account");
    let request = EntFaucetRequest {
        account_id: account_id.to_hex(),
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
        Json(request),
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
}
