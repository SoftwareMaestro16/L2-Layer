use super::{workers::produce_block_once, ApiError, AppState};
use crate::faucet::{EntFaucetRequest, EntFaucetResponse, EntFaucetService};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use l2_core::{crypto::sha256_bytes, DepositEvent, Hash32};

pub(super) async fn admin_deposit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(deposit): Json<DepositEvent>,
) -> Result<StatusCode, ApiError> {
    state.admin_auth.authorize(&headers)?;
    if !state.dev_admin_deposits_enabled {
        return Err(ApiError::forbidden("dev admin deposits disabled"));
    }
    validate_deposit_event(&deposit)?;

    let inserted = state.storage.save_deposit(deposit.clone()).await?;
    if inserted {
        let mut sequencer = state.sequencer.write().await;
        sequencer.ingest_deposits(vec![deposit]);
    }
    Ok(StatusCode::ACCEPTED)
}

pub(super) async fn admin_produce_block(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    state.admin_auth.authorize(&headers)?;

    Ok(match produce_block_once(&state).await? {
        Some(block) => (StatusCode::CREATED, Json(block)).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

pub(super) async fn admin_ent_faucet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EntFaucetRequest>,
) -> Result<Json<EntFaucetResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let account_id = EntFaucetService::parse_account_id(&request.account_id)
        .map_err(|_| ApiError::bad_request("invalid account id"))?;
    let grant = state.ent_faucet.grant(&state.storage, account_id).await?;

    if let Some(deposit) = grant.deposit {
        let mut sequencer = state.sequencer.write().await;
        sequencer.ingest_deposits(vec![deposit]);
    }

    Ok(Json(grant.response))
}

#[allow(dead_code)]
fn dev_deposit_id(seed: &str) -> Hash32 {
    sha256_bytes(seed.as_bytes())
}

pub(super) fn validate_deposit_event(deposit: &DepositEvent) -> Result<(), ApiError> {
    if deposit.deposit_id == Hash32::ZERO {
        return Err(ApiError::bad_request("deposit id must be non-zero"));
    }
    if deposit.recipient == Hash32::ZERO {
        return Err(ApiError::bad_request("recipient must be non-zero"));
    }
    if deposit.amount == 0 {
        return Err(ApiError::bad_request("amount must be non-zero"));
    }
    if deposit.l1_tx_hash == Hash32::ZERO {
        return Err(ApiError::bad_request("l1 tx hash must be non-zero"));
    }
    if deposit.l1_lt == 0 {
        return Err(ApiError::bad_request("l1 logical time must be non-zero"));
    }

    Ok(())
}
