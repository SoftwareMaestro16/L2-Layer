use super::{ApiError, AppState};
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{
    l2_raw_address, l2_user_friendly_address, parse_l2_address, read_sample_counter_value, Hash32,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct SampleCounterResponse {
    pub(super) contract: Hash32,
    pub(super) contract_raw_address: String,
    pub(super) contract_friendly_address: String,
    pub(super) counter: u64,
    pub(super) code_hash: Hash32,
    pub(super) data_hash: Hash32,
    pub(super) storage_root: Hash32,
}

#[derive(Debug, Serialize)]
pub(super) struct ContractGetMethodResponse {
    pub(super) contract: Hash32,
    pub(super) contract_raw_address: String,
    pub(super) contract_friendly_address: String,
    pub(super) method: String,
    pub(super) result: serde_json::Value,
    pub(super) source: &'static str,
}

pub(super) async fn get_sample_counter(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SampleCounterResponse>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    let counter = read_sample_counter_value(account)
        .map_err(|_| ApiError::bad_request("not a sample counter contract"))?;
    Ok(Json(SampleCounterResponse {
        contract: id,
        contract_raw_address: l2_raw_address(id),
        contract_friendly_address: l2_user_friendly_address(id),
        counter,
        code_hash: account.code_hash,
        data_hash: account.data_hash,
        storage_root: account.storage_root,
    }))
}

pub(super) async fn get_contract_method(
    State(state): State<AppState>,
    Path((id, method)): Path<(String, String)>,
) -> Result<Json<ContractGetMethodResponse>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .ok_or_else(|| ApiError::not_found("account not found"))?;

    if method == "currentCounter" || method == "counter" {
        let counter = read_sample_counter_value(account)
            .map_err(|_| ApiError::bad_request("not a sample counter contract"))?;
        return Ok(Json(ContractGetMethodResponse {
            contract: id,
            contract_raw_address: l2_raw_address(id),
            contract_friendly_address: l2_user_friendly_address(id),
            method,
            result: serde_json::json!({
                "type": "uint64",
                "value": counter.to_string(),
            }),
            source: "l2_state",
        }));
    }

    Err(ApiError::bad_request("get method not implemented"))
}
