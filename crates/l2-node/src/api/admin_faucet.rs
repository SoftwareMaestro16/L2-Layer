use super::{ApiError, AppState};
use crate::faucet::{
    EntFaucetBatchClaimRequest, EntFaucetBatchClaimResponse, EntFaucetBatchClaimStatus,
    EntFaucetBatchRequest, EntFaucetBatchResponse, EntFaucetBatchTotals, EntFaucetRequest,
    EntFaucetResponse, EntFaucetService, FaucetError,
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
        let (response, deposit) = process_batch_claim(&state, claim).await;
        if let Some(deposit) = deposit {
            deposits.push(deposit);
        }
        claims.push(response);
    }

    ingest_granted_deposits(&state, deposits).await;
    let totals = EntFaucetBatchTotals::from_claims(&claims);
    Ok(Json(EntFaucetBatchResponse { claims, totals }))
}

async fn process_batch_claim(
    state: &AppState,
    claim: EntFaucetBatchClaimRequest,
) -> (EntFaucetBatchClaimResponse, Option<DepositEvent>) {
    let amount_ent = claim
        .amount_ent
        .unwrap_or_else(|| state.ent_faucet.default_amount_ent());
    let claim_id = match EntFaucetService::parse_claim_id(&claim.claim_id) {
        Ok(claim_id) => claim_id,
        Err(_) => {
            return (
                failed_claim(None, None, amount_ent, "invalid_claim_id"),
                None,
            )
        }
    };
    let account_id = match EntFaucetService::parse_account_id(&claim.account_id) {
        Ok(account_id) => account_id,
        Err(_) => {
            return (
                failed_claim(Some(claim_id), None, amount_ent, "invalid_account_id")
                    .with_status(EntFaucetBatchClaimStatus::InvalidAccount),
                None,
            )
        }
    };
    match state
        .ent_faucet
        .grant_claim(&state.storage, claim_id, account_id, claim.amount_ent)
        .await
    {
        Ok(grant) => {
            let response = EntFaucetBatchClaimResponse {
                claim_id: Some(claim_id),
                account_id: Some(account_id),
                account_raw_address: Some(grant.response.account_raw_address),
                account_friendly_address: Some(grant.response.account_friendly_address),
                amount_ent: grant.response.amount_ent,
                amount_base_units: grant.response.amount_base_units,
                deposit_id: Some(grant.response.deposit_id),
                status: grant.status,
                error_code: error_code_for_status(grant.status).map(str::to_owned),
            };
            (response, grant.deposit)
        }
        Err(error) => (
            failed_claim(
                Some(claim_id),
                Some(account_id),
                amount_ent,
                error_code_for_error(&error),
            )
            .with_status(status_for_error(&error)),
            None,
        ),
    }
}

async fn ingest_granted_deposits(state: &AppState, deposits: Vec<DepositEvent>) {
    if deposits.is_empty() {
        return;
    }
    let mut sequencer = state.sequencer.write().await;
    sequencer.ingest_deposits(deposits);
}

fn failed_claim(
    claim_id: Option<l2_core::Hash32>,
    account_id: Option<l2_core::Hash32>,
    amount_ent: u128,
    error_code: &'static str,
) -> EntFaucetBatchClaimResponse {
    EntFaucetBatchClaimResponse {
        claim_id,
        account_id,
        account_raw_address: account_id.map(l2_core::l2_raw_address),
        account_friendly_address: account_id.map(l2_core::l2_user_friendly_address),
        amount_ent,
        amount_base_units: 0,
        deposit_id: None,
        status: EntFaucetBatchClaimStatus::Failed,
        error_code: Some(error_code.to_owned()),
    }
}

fn status_for_error(error: &FaucetError) -> EntFaucetBatchClaimStatus {
    match error {
        FaucetError::InvalidAccountId | FaucetError::ZeroAccountId => {
            EntFaucetBatchClaimStatus::InvalidAccount
        }
        _ => EntFaucetBatchClaimStatus::Failed,
    }
}

fn error_code_for_error(error: &FaucetError) -> &'static str {
    match error {
        FaucetError::InvalidAccountId => "invalid_account_id",
        FaucetError::InvalidClaimId => "invalid_claim_id",
        FaucetError::ZeroAccountId => "reserved_zero_address",
        FaucetError::ClaimConflict => "claim_conflict",
        FaucetError::InvalidAmount => "invalid_amount",
        FaucetError::AmountTooHigh => "amount_exceeds_max",
        FaucetError::AmountOverflow => "amount_overflow",
        FaucetError::Storage(_) => "storage_error",
    }
}

fn error_code_for_status(status: EntFaucetBatchClaimStatus) -> Option<&'static str> {
    match status {
        EntFaucetBatchClaimStatus::Granted => None,
        EntFaucetBatchClaimStatus::DuplicateClaim => Some("duplicate_claim"),
        EntFaucetBatchClaimStatus::DuplicateAccount => Some("duplicate_account"),
        EntFaucetBatchClaimStatus::InvalidAccount => Some("invalid_account"),
        EntFaucetBatchClaimStatus::Failed => Some("failed"),
    }
}

trait FaucetClaimResponseExt {
    fn with_status(self, status: EntFaucetBatchClaimStatus) -> Self;
}

impl FaucetClaimResponseExt for EntFaucetBatchClaimResponse {
    fn with_status(mut self, status: EntFaucetBatchClaimStatus) -> Self {
        self.status = status;
        self
    }
}
