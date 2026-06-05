use super::*;
use crate::faucet::{EntFaucetBatchClaimRequest, EntFaucetBatchClaimStatus, EntFaucetBatchRequest};
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use l2_core::crypto::sha256_bytes;
use l2_core::{l2_raw_address, l2_user_friendly_address, Hash32};

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

#[tokio::test]
async fn batch_requires_authorization() {
    let state = test_state(None);
    let request = EntFaucetBatchRequest {
        claims: vec![EntFaucetBatchClaimRequest {
            claim_id: sha256_bytes(b"claim").to_hex(),
            account_id: l2_raw_address(sha256_bytes(b"account")),
            amount_ent: None,
        }],
    };

    let error = admin_ent_faucet_batch(State(state), auth_headers(ADMIN_TOKEN), Json(request))
        .await
        .unwrap_err();

    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn batch_is_idempotent_by_claim_id() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"batch-account");
    let claim_id = sha256_bytes(b"claim-one");
    let request = EntFaucetBatchRequest {
        claims: vec![EntFaucetBatchClaimRequest {
            claim_id: claim_id.to_hex(),
            account_id: l2_user_friendly_address(account_id),
            amount_ent: None,
        }],
    };

    let first = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(request.clone()),
    )
    .await
    .expect("first batch");
    assert_eq!(first.0.claims[0].status, EntFaucetBatchClaimStatus::Granted);
    assert_eq!(first.0.totals.granted, 1);

    let duplicate = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(request),
    )
    .await
    .expect("duplicate batch");
    assert_eq!(
        duplicate.0.claims[0].status,
        EntFaucetBatchClaimStatus::DuplicateClaim
    );
    assert_eq!(
        duplicate.0.claims[0].deposit_id,
        first.0.claims[0].deposit_id
    );
    assert_eq!(duplicate.0.totals.duplicate_claim, 1);
}

#[tokio::test]
async fn batch_reports_duplicate_account_without_double_credit() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"repeat-account");
    let request = EntFaucetBatchRequest {
        claims: vec![
            EntFaucetBatchClaimRequest {
                claim_id: sha256_bytes(b"claim-a").to_hex(),
                account_id: l2_raw_address(account_id),
                amount_ent: None,
            },
            EntFaucetBatchClaimRequest {
                claim_id: sha256_bytes(b"claim-b").to_hex(),
                account_id: l2_raw_address(account_id),
                amount_ent: None,
            },
        ],
    };

    let response = admin_ent_faucet_batch(State(state), auth_headers(ADMIN_TOKEN), Json(request))
        .await
        .expect("batch grants");

    assert_eq!(
        response.0.claims[0].status,
        EntFaucetBatchClaimStatus::Granted
    );
    assert_eq!(
        response.0.claims[1].status,
        EntFaucetBatchClaimStatus::DuplicateAccount
    );
    assert_eq!(response.0.totals.granted, 1);
    assert_eq!(response.0.totals.duplicate_account, 1);
}

#[tokio::test]
async fn batch_reports_invalid_and_conflicting_claims_per_item() {
    let state = test_state(Some(ADMIN_TOKEN));
    let claim_id = sha256_bytes(b"claim-conflict");

    let zero_response = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![EntFaucetBatchClaimRequest {
                claim_id: sha256_bytes(b"zero-claim").to_hex(),
                account_id: l2_raw_address(Hash32::ZERO),
                amount_ent: None,
            }],
        }),
    )
    .await
    .expect("zero address is a per-claim failure");
    assert_eq!(
        zero_response.0.claims[0].status,
        EntFaucetBatchClaimStatus::InvalidAccount
    );
    assert_eq!(
        zero_response.0.claims[0].error_code.as_deref(),
        Some("reserved_zero_address")
    );

    let _ = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![EntFaucetBatchClaimRequest {
                claim_id: claim_id.to_hex(),
                account_id: l2_raw_address(sha256_bytes(b"first-account")),
                amount_ent: None,
            }],
        }),
    )
    .await
    .expect("first claim");

    let conflict = admin_ent_faucet_batch(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![EntFaucetBatchClaimRequest {
                claim_id: claim_id.to_hex(),
                account_id: l2_raw_address(sha256_bytes(b"second-account")),
                amount_ent: None,
            }],
        }),
    )
    .await
    .expect("claim conflict is a per-claim failure");
    assert_eq!(
        conflict.0.claims[0].status,
        EntFaucetBatchClaimStatus::Failed
    );
    assert_eq!(
        conflict.0.claims[0].error_code.as_deref(),
        Some("claim_conflict")
    );
    assert_eq!(conflict.0.totals.failed, 1);
}

#[tokio::test]
async fn batch_supports_bounded_custom_amounts() {
    let state = test_state(Some(ADMIN_TOKEN));
    let account_id = sha256_bytes(b"small-amount-account");

    let response = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![EntFaucetBatchClaimRequest {
                claim_id: sha256_bytes(b"small-amount").to_hex(),
                account_id: l2_raw_address(account_id),
                amount_ent: Some(100),
            }],
        }),
    )
    .await
    .expect("small amount");
    assert_eq!(
        response.0.claims[0].status,
        EntFaucetBatchClaimStatus::Granted
    );
    assert_eq!(response.0.claims[0].amount_ent, 100);
    assert_eq!(response.0.claims[0].amount_base_units, 100_000_000_000);

    let too_high = admin_ent_faucet_batch(
        State(state),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![EntFaucetBatchClaimRequest {
                claim_id: sha256_bytes(b"too-high").to_hex(),
                account_id: l2_raw_address(sha256_bytes(b"too-high-account")),
                amount_ent: Some(1_001),
            }],
        }),
    )
    .await
    .expect("too high is per-claim failure");
    assert_eq!(
        too_high.0.claims[0].status,
        EntFaucetBatchClaimStatus::Failed
    );
    assert_eq!(
        too_high.0.claims[0].error_code.as_deref(),
        Some("amount_exceeds_max")
    );
}

#[tokio::test]
async fn batch_partial_failure_does_not_block_successful_claims() {
    let state = test_state(Some(ADMIN_TOKEN));
    let valid_account = sha256_bytes(b"valid-partial-account");

    let response = admin_ent_faucet_batch(
        State(state.clone()),
        auth_headers(ADMIN_TOKEN),
        Json(EntFaucetBatchRequest {
            claims: vec![
                EntFaucetBatchClaimRequest {
                    claim_id: sha256_bytes(b"valid-partial").to_hex(),
                    account_id: l2_raw_address(valid_account),
                    amount_ent: None,
                },
                EntFaucetBatchClaimRequest {
                    claim_id: sha256_bytes(b"invalid-partial").to_hex(),
                    account_id: "not-an-account".to_owned(),
                    amount_ent: None,
                },
            ],
        }),
    )
    .await
    .expect("partial batch");

    assert_eq!(response.0.totals.granted, 1);
    assert_eq!(response.0.totals.invalid_account, 1);
    produce_block_once(&state)
        .await
        .expect("storage")
        .expect("faucet block");
    let sequencer = state.sequencer.read().await;
    assert_eq!(
        sequencer.state.account(valid_account).unwrap().balance(0),
        1_000_000_000_000
    );
}
