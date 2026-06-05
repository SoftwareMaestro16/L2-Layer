use super::{ApiError, AppState};
use crate::storage::StoredContractState;
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{l2_raw_address, l2_user_friendly_address, parse_l2_address, Account, Hash32};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct ContractStateResponse {
    pub(super) contract: Hash32,
    pub(super) contract_raw_address: String,
    pub(super) contract_friendly_address: String,
    pub(super) account: Account,
    pub(super) code: ContractCodeCellResponse,
    pub(super) data: ContractDataCellResponse,
    pub(super) last_block_height: u64,
    pub(super) source: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct ContractCodeCellResponse {
    pub(super) code_hash: Hash32,
    pub(super) code_boc_base64: String,
    pub(super) size_bytes: usize,
    pub(super) first_seen_block_height: u64,
}

#[derive(Debug, Serialize)]
pub(super) struct ContractDataCellResponse {
    pub(super) data_hash: Hash32,
    pub(super) storage_root: Hash32,
    pub(super) data_boc_base64: String,
    pub(super) size_bytes: usize,
    pub(super) first_seen_block_height: u64,
}

pub(super) async fn get_contract_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ContractStateResponse>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    {
        let sequencer = state.sequencer.read().await;
        if let Some(account) = sequencer.state.account(id) {
            if let Some(record) = StoredContractState::from_account(id, account, account.last_lt)? {
                return Ok(Json(contract_state_response(record, "l2_state")));
            }
        }
    }

    let record = state
        .storage
        .get_contract_state(id)
        .await?
        .ok_or_else(|| ApiError::not_found("contract state not found"))?;
    Ok(Json(contract_state_response(record, "storage_registry")))
}

fn contract_state_response(
    record: StoredContractState,
    source: &'static str,
) -> ContractStateResponse {
    ContractStateResponse {
        contract: record.account_id,
        contract_raw_address: l2_raw_address(record.account_id),
        contract_friendly_address: l2_user_friendly_address(record.account_id),
        account: record.account,
        code: ContractCodeCellResponse {
            code_hash: record.code_cell.code_hash,
            code_boc_base64: record.code_cell.code_boc_base64,
            size_bytes: record.code_cell.size_bytes,
            first_seen_block_height: record.code_cell.first_seen_block_height,
        },
        data: ContractDataCellResponse {
            data_hash: record.data_cell.data_hash,
            storage_root: record.data_cell.storage_root,
            data_boc_base64: record.data_cell.data_boc_base64,
            size_bytes: record.data_cell.size_bytes,
            first_seen_block_height: record.data_cell.first_seen_block_height,
        },
        last_block_height: record.last_block_height,
        source,
    }
}
