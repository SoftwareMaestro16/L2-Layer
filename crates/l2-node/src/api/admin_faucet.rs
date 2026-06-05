use super::{ApiError, AppState};
use crate::faucet::{
    EntFaucetBatchClaimResponse, EntFaucetBatchRequest, EntFaucetBatchResponse, EntFaucetRequest,
    EntFaucetResponse, EntFaucetService,
};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use l2_core::DepositEvent;

const MAX_BATCH_CLAIMS: usize = 100;

pub(in crate::api) async fn admin_ent_faucet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EntFaucetRequest>,
) -> Result<Json<EntFaucetResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let account_id = EntFaucetService::parse_account_id(&request.account_id)
        .map_err(|_| ApiError::bad_request("invalid account id"))?;
    let grant = state.ent_faucet.grant(&state.storage, account_id).await?;

    ingest_granted_deposits(&state, grant.deposit.into_iter().collect()).await;
    Ok(Json(grant.response))
}

pub(in crate::api) async fn admin_ent_faucet_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EntFaucetBatchRequest>,
) -> Result<Json<EntFaucetBatchResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    if request.claims.is_empty() {
        return Err(ApiError::bad_request("claims are required"));
    }
    if request.claims.len() > MAX_BATCH_CLAIMS {
        return Err(ApiError::bad_request("too many faucet claims"));
    }

    let mut deposits = Vec::new();
    let mut claims = Vec::with_capacity(request.claims.len());
    for claim in request.claims {
        let claim_id = EntFaucetService::parse_claim_id(&claim.claim_id)?;
        let account_id = EntFaucetService::parse_account_id(&claim.account_id)?;
        let grant = state
            .ent_faucet
            .grant_claim(&state.storage, claim_id, account_id)
            .await?;
        if let Some(deposit) = grant.deposit {
            deposits.push(deposit);
        }
        claims.push(EntFaucetBatchClaimResponse {
            claim_id,
            faucet: grant.response,
        });
    }

    ingest_granted_deposits(&state, deposits).await;
    Ok(Json(EntFaucetBatchResponse { claims }))
}

async fn ingest_granted_deposits(state: &AppState, deposits: Vec<DepositEvent>) {
    if deposits.is_empty() {
        return;
    }
    let mut sequencer = state.sequencer.write().await;
    sequencer.ingest_deposits(deposits);
}
