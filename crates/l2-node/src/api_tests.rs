use super::*;
use axum::http::HeaderValue;

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
