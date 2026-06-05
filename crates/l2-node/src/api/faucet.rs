use super::{ApiError, AppState};
use crate::faucet::{
    EntFaucetRequest, EntFaucetResponse, EntFaucetService, MAX_ENT_FAUCET_BATCH_CLAIMS,
};
use crate::storage::{EntFaucetClaimRecord, EntFaucetClaimSaveStatus, EntFaucetClaimStatus};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use l2_core::{l2_raw_address, l2_user_friendly_address, Hash32};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub(super) struct EntFaucetBatchRequest {
    pub(super) claims: Vec<EntFaucetBatchClaimRequest>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct EntFaucetBatchClaimRequest {
    pub(super) claim_id: String,
    pub(super) account_id: String,
    #[serde(default, deserialize_with = "optional_u128::deserialize")]
    pub(super) amount_ent: Option<u128>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct EntFaucetBatchResponse {
    pub(super) batch_id: Hash32,
    pub(super) totals: EntFaucetBatchTotals,
    pub(super) claims: Vec<EntFaucetBatchClaimResponse>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct EntFaucetBatchTotals {
    pub(super) total: usize,
    pub(super) granted: usize,
    pub(super) duplicate_claim: usize,
    pub(super) duplicate_account: usize,
    pub(super) invalid_account: usize,
    pub(super) invalid_amount: usize,
    pub(super) failed: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct EntFaucetBatchClaimResponse {
    pub(super) claim_id: String,
    pub(super) status: String,
    pub(super) account_id: Option<Hash32>,
    pub(super) account_raw_address: Option<String>,
    pub(super) account_friendly_address: Option<String>,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(super) amount_ent: u128,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(super) amount_base_units: u128,
    pub(super) deposit_id: Option<Hash32>,
    pub(super) error: Option<&'static str>,
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
        state.sequencer.write().await.ingest_deposits(vec![deposit]);
    }

    Ok(Json(grant.response))
}

pub(super) async fn admin_ent_faucet_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<EntFaucetBatchRequest>,
) -> Result<Json<EntFaucetBatchResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    if request.claims.is_empty() {
        return Err(ApiError::bad_request("empty faucet batch"));
    }
    if request.claims.len() > MAX_ENT_FAUCET_BATCH_CLAIMS {
        return Err(ApiError::bad_request("faucet batch too large"));
    }

    let batch_id =
        EntFaucetService::batch_id(request.claims.iter().map(|claim| claim.claim_id.trim()));
    let mut deposits = Vec::new();
    let mut claims = Vec::with_capacity(request.claims.len());
    for (index, claim) in request.claims.into_iter().enumerate() {
        claims.push(process_claim(&state, batch_id, index as u32, claim, &mut deposits).await?);
    }
    if !deposits.is_empty() {
        state.sequencer.write().await.ingest_deposits(deposits);
    }

    Ok(Json(EntFaucetBatchResponse {
        batch_id,
        totals: batch_totals(&claims),
        claims,
    }))
}

async fn process_claim(
    state: &AppState,
    batch_id: Hash32,
    claim_index: u32,
    claim: EntFaucetBatchClaimRequest,
    deposits: &mut Vec<l2_core::DepositEvent>,
) -> Result<EntFaucetBatchClaimResponse, ApiError> {
    let claim_id = claim.claim_id.trim().to_owned();
    if let Err(error) = EntFaucetService::validate_claim_id(&claim_id) {
        return Ok(batch_error_response(claim_id, "failed", error.into()));
    }
    let amount_ent = claim
        .amount_ent
        .unwrap_or_else(|| state.ent_faucet.default_amount_ent());
    let amount_base_units = match state.ent_faucet.amount_ent_to_base_units(amount_ent) {
        Ok(amount) => amount,
        Err(error) => {
            return Ok(batch_error_response(
                claim_id,
                "invalid_amount",
                error.into(),
            ))
        }
    };
    let account_id = match EntFaucetService::parse_account_id(&claim.account_id) {
        Ok(account_id) if account_id != Hash32::ZERO => account_id,
        _ => {
            return Ok(batch_error_response(
                claim_id,
                "invalid_account",
                ApiError::bad_request("invalid account id"),
            ))
        }
    };

    let deposit = state
        .ent_faucet
        .batch_deposit_event(&claim_id, account_id, amount_base_units);
    let result = state
        .storage
        .save_ent_faucet_batch_claim(
            EntFaucetClaimRecord {
                batch_id,
                claim_index,
                claim_id: claim_id.clone(),
                account_id,
                amount_base_units,
                deposit_id: deposit.deposit_id,
                status: EntFaucetClaimStatus::Granted,
            },
            deposit.clone(),
        )
        .await?;
    if result.status == EntFaucetClaimSaveStatus::Granted {
        deposits.push(deposit);
    }

    let amount_ent = state
        .ent_faucet
        .amount_base_units_to_ent(result.record.amount_base_units)
        .unwrap_or(amount_ent);
    Ok(batch_success_response(
        result.status.as_str(),
        amount_ent,
        result.record,
    ))
}

fn batch_success_response(
    status: &str,
    amount_ent: u128,
    record: EntFaucetClaimRecord,
) -> EntFaucetBatchClaimResponse {
    EntFaucetBatchClaimResponse {
        claim_id: record.claim_id,
        status: status.to_owned(),
        account_id: Some(record.account_id),
        account_raw_address: Some(l2_raw_address(record.account_id)),
        account_friendly_address: Some(l2_user_friendly_address(record.account_id)),
        amount_ent,
        amount_base_units: record.amount_base_units,
        deposit_id: Some(record.deposit_id),
        error: None,
    }
}

fn batch_error_response(
    claim_id: String,
    status: &str,
    error: ApiError,
) -> EntFaucetBatchClaimResponse {
    EntFaucetBatchClaimResponse {
        claim_id,
        status: status.to_owned(),
        account_id: None,
        account_raw_address: None,
        account_friendly_address: None,
        amount_ent: 0,
        amount_base_units: 0,
        deposit_id: None,
        error: Some(match error.message.as_str() {
            "invalid faucet amount" => "invalid_amount",
            "invalid claim id" => "invalid_claim_id",
            "reserved zero address" | "invalid account id" => "invalid_account",
            _ => "failed",
        }),
    }
}

fn batch_totals(claims: &[EntFaucetBatchClaimResponse]) -> EntFaucetBatchTotals {
    let mut totals = EntFaucetBatchTotals {
        total: claims.len(),
        ..EntFaucetBatchTotals::default()
    };
    for claim in claims {
        match claim.status.as_str() {
            "granted" => totals.granted += 1,
            "duplicate_claim" => totals.duplicate_claim += 1,
            "duplicate_account" => totals.duplicate_account += 1,
            "invalid_account" => totals.invalid_account += 1,
            "invalid_amount" => totals.invalid_amount += 1,
            _ => totals.failed += 1,
        }
    }
    totals
}

mod optional_u128 {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer};

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
            return Ok(None);
        };
        match value {
            serde_json::Value::Number(number) => number
                .as_u64()
                .map(|value| Some(value as u128))
                .ok_or_else(|| D::Error::custom("amount_ent must be an unsigned integer")),
            serde_json::Value::String(value) => {
                value.parse::<u128>().map(Some).map_err(D::Error::custom)
            }
            _ => Err(D::Error::custom("amount_ent must be a string or integer")),
        }
    }
}
