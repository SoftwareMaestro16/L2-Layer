mod tx_view;

#[cfg(test)]
pub(in crate::api) use tx_view::ExplorerAccountTransactionsQuery;
pub(in crate::api) use tx_view::{explorer_account_transactions, explorer_tx};

use super::super::{ApiError, AppState};
use crate::storage::{
    StoredContractState, VerifierSourceFile, VerifierStatus, VerifierSubmissionRecord,
};
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{
    decode_contract_cell_boc_base64, interface_for_code_hash, is_l2_zero_address, l2_raw_address,
    l2_user_friendly_address, parse_l2_address, Hash32, DEFAULT_MAX_TVM_BOC_BYTES,
    L2_ZERO_ADDRESS_INTERFACE, L2_ZERO_ADDRESS_LABEL,
};
use serde::{Deserialize, Serialize};

const MAX_VERIFIER_FILES: usize = 16;
const MAX_VERIFIER_FILE_BYTES: usize = 128 * 1024;
const MAX_VERIFIER_TOTAL_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerAccount {
    pub(in crate::api) account_id: Hash32,
    pub(in crate::api) raw_address: String,
    pub(in crate::api) user_friendly_address: String,
    pub(in crate::api) status: &'static str,
    pub(in crate::api) nonce: u64,
    pub(in crate::api) balances: Vec<ExplorerBalance>,
    pub(in crate::api) code_hash: Hash32,
    pub(in crate::api) data_hash: Hash32,
    pub(in crate::api) storage_root: Hash32,
    pub(in crate::api) interfaces: Vec<ExplorerInterface>,
    pub(in crate::api) last_lt: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerInterface {
    pub(in crate::api) id: &'static str,
    pub(in crate::api) label: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerBalance {
    pub(in crate::api) asset_id: u32,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(in crate::api) amount: u128,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerAccountAssets {
    pub(in crate::api) tokens: Vec<ExplorerTokenHolding>,
    pub(in crate::api) collectibles: Vec<ExplorerCollectible>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerTokenHolding {
    pub(in crate::api) id: String,
    pub(in crate::api) asset_id: u32,
    pub(in crate::api) symbol: String,
    pub(in crate::api) name: String,
    pub(in crate::api) decimals: u8,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(in crate::api) amount: u128,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerCollectible {
    pub(in crate::api) id: String,
    pub(in crate::api) name: String,
    pub(in crate::api) collection: String,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerAccountCode {
    pub(in crate::api) account_id: Hash32,
    pub(in crate::api) bytecode: ExplorerCellView,
    pub(in crate::api) raw_data: ExplorerCellView,
    pub(in crate::api) source: ExplorerCodeSource,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerCellView {
    pub(in crate::api) hex: String,
    pub(in crate::api) base64: String,
    pub(in crate::api) hex_hash: String,
    pub(in crate::api) root_hash: Hash32,
    pub(in crate::api) size_bytes: usize,
    pub(in crate::api) cell_count: usize,
    pub(in crate::api) cells: Vec<ExplorerCellSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerCellSummary {
    pub(in crate::api) index: usize,
    pub(in crate::api) role: &'static str,
    pub(in crate::api) hash: Hash32,
    pub(in crate::api) size_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerCodeSource {
    pub(in crate::api) status: &'static str,
    pub(in crate::api) code_hash: Hash32,
    pub(in crate::api) submission_id: Option<Hash32>,
    pub(in crate::api) files: Vec<VerifierSourceFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::api) struct VerifierSubmitRequest {
    pub(in crate::api) account_id: Option<String>,
    pub(in crate::api) code_hash: Option<String>,
    pub(in crate::api) files: Vec<VerifierSourceFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::api) struct VerifierReviewRequest {
    pub(in crate::api) status: String,
}

pub(in crate::api) async fn explorer_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerAccount>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    if is_l2_zero_address(id) {
        return Ok(Json(ExplorerAccount {
            account_id: id,
            raw_address: l2_raw_address(id),
            user_friendly_address: l2_user_friendly_address(id),
            status: "reserved",
            nonce: 0,
            balances: vec![],
            code_hash: Hash32::ZERO,
            data_hash: Hash32::ZERO,
            storage_root: Hash32::ZERO,
            interfaces: vec![ExplorerInterface {
                id: L2_ZERO_ADDRESS_INTERFACE,
                label: L2_ZERO_ADDRESS_LABEL,
            }],
            last_lt: 0,
        }));
    }

    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("account not found"))?;

    Ok(Json(ExplorerAccount {
        account_id: id,
        raw_address: l2_raw_address(id),
        user_friendly_address: l2_user_friendly_address(id),
        status: "active",
        nonce: account.nonce,
        balances: account
            .balances
            .into_iter()
            .map(|(asset_id, amount)| ExplorerBalance { asset_id, amount })
            .collect(),
        code_hash: account.code_hash,
        data_hash: account.data_hash,
        storage_root: account.storage_root,
        interfaces: account_interfaces(account.code_hash),
        last_lt: account.last_lt,
    }))
}

pub(in crate::api) async fn explorer_account_assets(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerAccountAssets>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    if is_l2_zero_address(id) {
        return Ok(Json(ExplorerAccountAssets {
            tokens: vec![],
            collectibles: vec![],
        }));
    }
    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    Ok(Json(ExplorerAccountAssets {
        tokens: account
            .balances
            .iter()
            .map(|(asset_id, amount)| token_holding(*asset_id, *amount))
            .collect(),
        collectibles: vec![],
    }))
}

pub(in crate::api) async fn explorer_account_code(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerAccountCode>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let record = load_contract_state(&state, id).await?;
    let source = source_response(&state, record.code_cell.code_hash).await?;
    Ok(Json(ExplorerAccountCode {
        account_id: id,
        bytecode: cell_view(
            &record.code_cell.code_boc_base64,
            record.code_cell.code_hash,
            "bytecode",
        )?,
        raw_data: cell_view(
            &record.data_cell.data_boc_base64,
            record.data_cell.data_hash,
            "raw_data",
        )?,
        source,
    }))
}

pub(in crate::api) async fn explorer_code_source(
    State(state): State<AppState>,
    Path(code_hash): Path<String>,
) -> Result<Json<ExplorerCodeSource>, ApiError> {
    let code_hash =
        Hash32::from_hex(&code_hash).map_err(|_| ApiError::bad_request("invalid code hash"))?;
    Ok(Json(source_response(&state, code_hash).await?))
}

pub(in crate::api) async fn explorer_verifier_submit(
    State(state): State<AppState>,
    Json(request): Json<VerifierSubmitRequest>,
) -> Result<Json<ExplorerCodeSource>, ApiError> {
    let account_id = request
        .account_id
        .as_deref()
        .map(parse_l2_address)
        .transpose()
        .map_err(|_| ApiError::bad_request("invalid account id"))?;
    let code_hash = match request.code_hash.as_deref() {
        Some(value) => {
            Hash32::from_hex(value).map_err(|_| ApiError::bad_request("invalid code hash"))?
        }
        None => {
            let Some(account_id) = account_id else {
                return Err(ApiError::bad_request("account_id or code_hash required"));
            };
            load_contract_state(&state, account_id)
                .await?
                .code_cell
                .code_hash
        }
    };
    validate_verifier_files(&request.files)?;
    let submission = VerifierSubmissionRecord {
        submission_id: verifier_submission_id(code_hash, &request.files),
        code_hash,
        account_id,
        status: VerifierStatus::Pending,
        files: request.files,
    };
    state.storage.save_verifier_submission(submission).await?;
    Ok(Json(source_response(&state, code_hash).await?))
}

pub(in crate::api) async fn admin_explorer_verifier_review(
    State(state): State<AppState>,
    Path(submission_id): Path<String>,
    Json(request): Json<VerifierReviewRequest>,
) -> Result<Json<ExplorerCodeSource>, ApiError> {
    let submission_id = Hash32::from_hex(&submission_id)
        .map_err(|_| ApiError::bad_request("invalid submission id"))?;
    let status = match VerifierStatus::parse(&request.status) {
        Some(VerifierStatus::Verified) => VerifierStatus::Verified,
        Some(VerifierStatus::Rejected) => VerifierStatus::Rejected,
        _ => return Err(ApiError::bad_request("status must be verified or rejected")),
    };
    let reviewed = state
        .storage
        .review_verifier_submission(submission_id, status)
        .await?
        .ok_or_else(|| ApiError::not_found("verifier submission not found"))?;
    Ok(Json(source_response(&state, reviewed.code_hash).await?))
}

fn account_interfaces(code_hash: Hash32) -> Vec<ExplorerInterface> {
    interface_for_code_hash(code_hash)
        .map(|(id, label)| ExplorerInterface { id, label })
        .into_iter()
        .collect()
}

fn token_holding(asset_id: u32, amount: u128) -> ExplorerTokenHolding {
    let (symbol, name, decimals) = match asset_id {
        0 => ("ENT".to_owned(), "Entropis".to_owned(), 9),
        1 => ("TON".to_owned(), "TON testnet".to_owned(), 9),
        other => (format!("A{other}"), format!("L2 asset {other}"), 9),
    };
    ExplorerTokenHolding {
        id: format!("asset-{asset_id}"),
        asset_id,
        symbol,
        name,
        decimals,
        amount,
    }
}

async fn load_contract_state(
    state: &AppState,
    account_id: Hash32,
) -> Result<StoredContractState, ApiError> {
    {
        let sequencer = state.sequencer.read().await;
        if let Some(account) = sequencer.state.account(account_id) {
            if let Some(record) =
                StoredContractState::from_account(account_id, account, account.last_lt)?
            {
                return Ok(record);
            }
        }
    }
    state
        .storage
        .get_contract_state(account_id)
        .await?
        .ok_or_else(|| ApiError::not_found("contract code not found"))
}

fn cell_view(
    boc_base64: &str,
    expected_hash: Hash32,
    role: &'static str,
) -> Result<ExplorerCellView, ApiError> {
    let cell = decode_contract_cell_boc_base64(boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
        .map_err(|_| ApiError::bad_request("malformed contract cell"))?;
    if cell.cell_hash != expected_hash {
        return Err(ApiError::conflict("contract cell hash mismatch"));
    }
    Ok(ExplorerCellView {
        hex: hex::encode(&cell.boc_bytes),
        base64: cell.boc_base64,
        hex_hash: cell.cell_hash.to_hex(),
        root_hash: cell.cell_hash,
        size_bytes: cell.boc_bytes.len(),
        cell_count: 1,
        cells: vec![ExplorerCellSummary {
            index: 0,
            role,
            hash: cell.cell_hash,
            size_bytes: cell.boc_bytes.len(),
        }],
    })
}

async fn source_response(
    state: &AppState,
    code_hash: Hash32,
) -> Result<ExplorerCodeSource, ApiError> {
    let Some(source) = state.storage.get_verifier_source(code_hash).await? else {
        return Ok(ExplorerCodeSource {
            status: "not_found",
            code_hash,
            submission_id: None,
            files: vec![],
        });
    };
    Ok(ExplorerCodeSource {
        status: source.status.as_str(),
        code_hash,
        submission_id: Some(source.submission_id),
        files: if source.status == VerifierStatus::Verified {
            source.files
        } else {
            vec![]
        },
    })
}

fn validate_verifier_files(files: &[VerifierSourceFile]) -> Result<(), ApiError> {
    if files.is_empty() {
        return Err(ApiError::bad_request("at least one .tolk file required"));
    }
    if files.len() > MAX_VERIFIER_FILES {
        return Err(ApiError::bad_request("too many source files"));
    }
    let mut total = 0usize;
    for file in files {
        if !file.path.ends_with(".tolk")
            || file.path.contains('\\')
            || file.path.contains("..")
            || file.path.starts_with('/')
        {
            return Err(ApiError::bad_request(
                "only relative .tolk files are accepted",
            ));
        }
        let bytes = file.content.as_bytes().len();
        if bytes == 0 || bytes > MAX_VERIFIER_FILE_BYTES {
            return Err(ApiError::bad_request("source file size is invalid"));
        }
        total = total
            .checked_add(bytes)
            .ok_or_else(|| ApiError::bad_request("source bundle too large"))?;
    }
    if total > MAX_VERIFIER_TOTAL_BYTES {
        return Err(ApiError::bad_request("source bundle too large"));
    }
    Ok(())
}

fn verifier_submission_id(code_hash: Hash32, files: &[VerifierSourceFile]) -> Hash32 {
    let mut material = Vec::new();
    material.extend_from_slice(code_hash.as_bytes());
    for file in files {
        material.extend_from_slice(file.path.as_bytes());
        material.push(0);
        material.extend_from_slice(file.content.as_bytes());
        material.push(0xff);
    }
    l2_core::crypto::hash_domain("entropis.explorer.verifier.v1", &[&material])
}
