use super::{ApiError, AppState};
use crate::storage::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus,
};
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{Hash32, L2Event, Receipt, ReceiptStatus, SignedL2Transaction};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct TxReceiptResponse {
    pub(in crate::api) tx_hash: Hash32,
    pub(in crate::api) status: &'static str,
    pub(in crate::api) transaction: Option<SignedL2Transaction>,
    pub(in crate::api) receipt: Option<TxReceiptDetail>,
    pub(in crate::api) block: Option<TxReceiptBlockRef>,
    pub(in crate::api) finality: Option<BlockFinalityResponse>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct TxReceiptDetail {
    pub(in crate::api) status: &'static str,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(in crate::api) gas_charged: u128,
    pub(in crate::api) reason: Option<String>,
    pub(in crate::api) withdrawal_id: Option<Hash32>,
    pub(in crate::api) events: Vec<L2Event>,
    pub(in crate::api) contract_logs: Vec<ContractLogDto>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ContractLogDto {
    pub(in crate::api) contract: Option<Hash32>,
    pub(in crate::api) level: &'static str,
    pub(in crate::api) message: String,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct TxReceiptBlockRef {
    pub(in crate::api) height: u64,
    pub(in crate::api) timestamp: u64,
    pub(in crate::api) block_hash: Hash32,
    pub(in crate::api) tx_index: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct BlockFinalityResponse {
    pub(in crate::api) block_height: u64,
    pub(in crate::api) block_hash: Hash32,
    pub(in crate::api) batch_no: u64,
    pub(in crate::api) committed: bool,
    pub(in crate::api) finalized: bool,
    pub(in crate::api) commit: Option<L1CommitStatusDto>,
    pub(in crate::api) finalization: Option<L1FinalizationStatusDto>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct L1CommitStatusDto {
    pub(in crate::api) status: &'static str,
    pub(in crate::api) attempts: u32,
    pub(in crate::api) message_hash: Option<Hash32>,
    pub(in crate::api) message_hash_norm: Option<Hash32>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct L1FinalizationStatusDto {
    pub(in crate::api) status: &'static str,
    pub(in crate::api) attempts: u32,
    pub(in crate::api) finalize_after_unix: u64,
    pub(in crate::api) message_hash: Option<Hash32>,
    pub(in crate::api) message_hash_norm: Option<Hash32>,
}

pub(super) async fn get_receipt(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<TxReceiptResponse>, ApiError> {
    tx_receipt_response(state, hash).await
}

pub(super) async fn get_tx_receipt(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<TxReceiptResponse>, ApiError> {
    tx_receipt_response(state, hash).await
}

pub(super) async fn get_block_finality(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<BlockFinalityResponse>, ApiError> {
    let block = state
        .storage
        .get_block(height)
        .await?
        .ok_or_else(|| ApiError::not_found("block not found"))?;
    Ok(Json(
        block_finality_response(&state, height, block.header.block_hash()).await?,
    ))
}

async fn tx_receipt_response(
    state: AppState,
    hash: String,
) -> Result<Json<TxReceiptResponse>, ApiError> {
    let tx_hash = Hash32::from_hex(&hash).map_err(|_| ApiError::bad_request("invalid tx hash"))?;
    if let Some(record) = state.storage.get_transaction(tx_hash).await? {
        let finality =
            block_finality_response(&state, record.block_height, record.block_hash).await?;
        let receipt = record.receipt.as_ref().map(receipt_detail);
        let status = lifecycle_status(record.receipt.as_ref(), &finality);
        return Ok(Json(TxReceiptResponse {
            tx_hash,
            status,
            transaction: Some(record.transaction),
            receipt,
            block: Some(TxReceiptBlockRef {
                height: record.block_height,
                timestamp: record.block_timestamp,
                block_hash: record.block_hash,
                tx_index: record.tx_index,
            }),
            finality: Some(finality),
        }));
    }

    if let Some(tx) = state.mempool.get_pending(tx_hash).await? {
        return Ok(Json(TxReceiptResponse {
            tx_hash,
            status: "pending",
            transaction: Some(tx),
            receipt: None,
            block: None,
            finality: None,
        }));
    }

    Err(ApiError::not_found("transaction not found"))
}

async fn block_finality_response(
    state: &AppState,
    block_height: u64,
    block_hash: Hash32,
) -> Result<BlockFinalityResponse, ApiError> {
    let batch_no = batch_no_from_block_height(block_height)?;
    let commit = state.storage.get_batch_commit(batch_no).await?;
    let finalization = state.storage.get_batch_finalization(batch_no).await?;
    let committed = commit
        .as_ref()
        .is_some_and(|record| record.status == BatchCommitStatus::Confirmed);
    let finalized = finalization
        .as_ref()
        .is_some_and(|record| record.status == BatchFinalizationStatus::Finalized);

    Ok(BlockFinalityResponse {
        block_height,
        block_hash,
        batch_no,
        committed,
        finalized,
        commit: commit.as_ref().map(commit_status),
        finalization: finalization.as_ref().map(finalization_status),
    })
}

fn lifecycle_status(receipt: Option<&Receipt>, finality: &BlockFinalityResponse) -> &'static str {
    if receipt.is_some_and(|receipt| receipt.status == ReceiptStatus::Rejected) {
        return "rejected";
    }
    if finality.finalized {
        return "finalized";
    }
    if finality.committed {
        return "committed";
    }
    "included"
}

fn receipt_detail(receipt: &Receipt) -> TxReceiptDetail {
    TxReceiptDetail {
        status: receipt_status(receipt),
        gas_charged: receipt.gas_charged,
        reason: receipt.reason.clone(),
        withdrawal_id: receipt.withdrawal_id,
        events: receipt.events.clone(),
        contract_logs: receipt.events.iter().map(contract_log).collect(),
    }
}

fn contract_log(event: &L2Event) -> ContractLogDto {
    ContractLogDto {
        contract: event.contract(),
        level: "info",
        message: event.kind().to_owned(),
    }
}

fn receipt_status(receipt: &Receipt) -> &'static str {
    match receipt.status {
        ReceiptStatus::Applied => "applied",
        ReceiptStatus::Rejected => "rejected",
    }
}

fn commit_status(record: &BatchCommitRecord) -> L1CommitStatusDto {
    L1CommitStatusDto {
        status: record.status.as_str(),
        attempts: record.attempts,
        message_hash: record.message_hash,
        message_hash_norm: record.message_hash_norm,
    }
}

fn finalization_status(record: &BatchFinalizationRecord) -> L1FinalizationStatusDto {
    L1FinalizationStatusDto {
        status: record.status.as_str(),
        attempts: record.attempts,
        finalize_after_unix: record.finalize_after_unix,
        message_hash: record.message_hash,
        message_hash_norm: record.message_hash_norm,
    }
}

fn batch_no_from_block_height(block_height: u64) -> Result<u64, ApiError> {
    block_height
        .checked_add(1)
        .ok_or_else(|| ApiError::bad_request("invalid block height"))
}
