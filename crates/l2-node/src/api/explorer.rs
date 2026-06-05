use super::{ApiError, AppState};
use crate::storage::{
    BatchCommitRecord, BatchFinalizationRecord, BatchFinalizationStatus, DynStorage,
};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::Json;
use l2_core::{Hash32, L2Block, L2TransactionKind, WithdrawalLeaf};
use serde::{Deserialize, Serialize};

pub(in crate::api) mod account;

pub(super) use account::{
    admin_explorer_verifier_review, explorer_account, explorer_account_assets,
    explorer_account_code, explorer_account_transactions, explorer_code_source, explorer_tx,
    explorer_verifier_submit,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;
const LOOKUP_BLOCK_LIMIT: usize = 500;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ExplorerListQuery {
    pub(super) limit: Option<usize>,
    pub(super) before_height: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerList<T> {
    pub(super) items: Vec<T>,
    pub(super) next_before_height: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerSummary {
    pub(super) latest_block: Option<ExplorerBlockSummary>,
    pub(super) latest_batch_commit: Option<ExplorerBatchStatus>,
    pub(super) latest_confirmed_commit: Option<ExplorerBatchStatus>,
    pub(super) latest_finalization: Option<ExplorerFinalizationStatus>,
    pub(super) latest_finalized_batch: Option<ExplorerFinalizationStatus>,
    pub(super) block_count: u64,
    pub(super) transaction_count: u64,
    pub(super) deposit_count: u64,
    pub(super) withdrawal_count: u64,
    pub(super) live_account_count: u64,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(super) live_ent_supply: u128,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerBlockSummary {
    pub(super) height: u64,
    pub(super) block_hash: Hash32,
    pub(super) timestamp: u64,
    pub(super) tx_count: usize,
    pub(super) deposit_count: usize,
    pub(super) withdrawal_count: usize,
    pub(super) state_root: Hash32,
    pub(super) data_hash: Hash32,
    pub(super) withdrawal_root: Hash32,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerDepositStatus {
    pub(super) status: &'static str,
    pub(super) block_height: u64,
    pub(super) tx_hash: Hash32,
    pub(super) deposit: ExplorerDeposit,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerDeposit {
    pub(super) deposit_id: Hash32,
    pub(super) asset_id: u32,
    pub(super) recipient: Hash32,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(super) amount: u128,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerWithdrawalStatus {
    pub(super) status: &'static str,
    pub(super) block_height: u64,
    pub(super) batch_no: u64,
    pub(super) proof_available: bool,
    pub(super) withdrawal_root: Hash32,
    pub(super) finalization: Option<ExplorerFinalizationStatus>,
    pub(super) leaf: WithdrawalLeaf,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerBatchStatus {
    pub(super) batch_no: u64,
    pub(super) block_height: u64,
    pub(super) block_hash: Hash32,
    pub(super) status: &'static str,
    pub(super) message_hash_norm: Option<Hash32>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct ExplorerFinalizationStatus {
    pub(super) batch_no: u64,
    pub(super) block_height: u64,
    pub(super) status: &'static str,
    pub(super) finalize_after_unix: u64,
    pub(super) message_hash_norm: Option<Hash32>,
}

pub(super) async fn explorer_summary(
    State(state): State<AppState>,
) -> Result<Json<ExplorerSummary>, ApiError> {
    let storage_stats = state.storage.explorer_storage_stats().await?;
    let latest_batch_commit = state.storage.latest_batch_commit(&[]).await?;
    let latest_height = latest_batch_commit
        .as_ref()
        .map(|record| record.block_height);
    let latest_block = match latest_height {
        Some(height) => state
            .storage
            .get_block(height)
            .await?
            .as_ref()
            .map(block_summary),
        None => None,
    };

    Ok(Json(ExplorerSummary {
        latest_block,
        latest_batch_commit: latest_batch_commit.as_ref().map(batch_status),
        latest_confirmed_commit: state
            .storage
            .latest_batch_commit(&[crate::storage::BatchCommitStatus::Confirmed])
            .await?
            .as_ref()
            .map(batch_status),
        latest_finalization: state
            .storage
            .latest_batch_finalization(&[])
            .await?
            .as_ref()
            .map(finalization_status),
        latest_finalized_batch: state
            .storage
            .latest_batch_finalization(&[BatchFinalizationStatus::Finalized])
            .await?
            .as_ref()
            .map(finalization_status),
        block_count: storage_stats.block_count,
        transaction_count: storage_stats.transaction_count,
        deposit_count: storage_stats.deposit_count,
        withdrawal_count: storage_stats.withdrawal_count,
        live_account_count: {
            let sequencer = state.sequencer.read().await;
            sequencer.state.accounts.len() as u64
        },
        live_ent_supply: {
            let sequencer = state.sequencer.read().await;
            sequencer
                .state
                .accounts
                .values()
                .map(|account| account.balance(l2_core::L2_NATIVE_GAS_ASSET))
                .sum()
        },
    }))
}

pub(super) async fn explorer_blocks(
    State(state): State<AppState>,
    Query(query): Query<ExplorerListQuery>,
) -> Result<Json<ExplorerList<ExplorerBlockSummary>>, ApiError> {
    let limit = bounded_limit(query.limit);
    let Some(start) = list_start_height(&state.storage, query.before_height).await? else {
        return Ok(Json(ExplorerList {
            items: vec![],
            next_before_height: None,
        }));
    };

    let blocks = scan_blocks_desc(&state.storage, start, limit).await?;
    let next_before_height = next_before_height(blocks.last().map(|block| block.header.height));
    Ok(Json(ExplorerList {
        items: blocks.iter().map(block_summary).collect(),
        next_before_height,
    }))
}

pub(super) async fn explorer_deposits(
    State(state): State<AppState>,
    Query(query): Query<ExplorerListQuery>,
) -> Result<Json<ExplorerList<ExplorerDepositStatus>>, ApiError> {
    let limit = bounded_limit(query.limit);
    let Some(start) = list_start_height(&state.storage, query.before_height).await? else {
        return Ok(Json(ExplorerList {
            items: vec![],
            next_before_height: None,
        }));
    };

    let mut deposits = Vec::new();
    let mut last_height = None;
    for block in scan_blocks_desc(&state.storage, start, LOOKUP_BLOCK_LIMIT).await? {
        last_height = Some(block.header.height);
        for transaction in &block.transactions {
            let L2TransactionKind::Deposit {
                deposit_id,
                asset_id,
                recipient,
                amount,
            } = &transaction.kind
            else {
                continue;
            };
            deposits.push(ExplorerDepositStatus {
                status: "included",
                block_height: block.header.height,
                tx_hash: transaction.tx_hash(),
                deposit: ExplorerDeposit {
                    deposit_id: *deposit_id,
                    asset_id: *asset_id,
                    recipient: *recipient,
                    amount: *amount,
                },
            });
            if deposits.len() >= limit {
                return Ok(Json(ExplorerList {
                    items: deposits,
                    next_before_height: next_before_height(last_height),
                }));
            }
        }
    }

    Ok(Json(ExplorerList {
        items: deposits,
        next_before_height: next_before_height(last_height),
    }))
}

pub(super) async fn explorer_deposit(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerDepositStatus>, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid deposit id"))?;
    let Some(latest) = list_start_height(&state.storage, None).await? else {
        return Err(ApiError::not_found("deposit not found"));
    };
    for block in scan_blocks_desc(&state.storage, latest, LOOKUP_BLOCK_LIMIT).await? {
        for transaction in &block.transactions {
            let L2TransactionKind::Deposit {
                deposit_id,
                asset_id,
                recipient,
                amount,
            } = &transaction.kind
            else {
                continue;
            };
            if *deposit_id == id {
                return Ok(Json(ExplorerDepositStatus {
                    status: "included",
                    block_height: block.header.height,
                    tx_hash: transaction.tx_hash(),
                    deposit: ExplorerDeposit {
                        deposit_id: *deposit_id,
                        asset_id: *asset_id,
                        recipient: *recipient,
                        amount: *amount,
                    },
                }));
            }
        }
    }

    Err(ApiError::not_found("deposit not found"))
}

pub(super) async fn explorer_withdrawal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerWithdrawalStatus>, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid withdrawal id"))?;
    let proof = state
        .storage
        .get_withdrawal_proof(id)
        .await?
        .ok_or_else(|| ApiError::not_found("withdrawal not found"))?;
    let batch_no = batch_no_from_block_height(proof.block_height)?;
    let finalization = state.storage.get_batch_finalization(batch_no).await?;
    let finalized = finalization
        .as_ref()
        .is_some_and(|record| record.status == BatchFinalizationStatus::Finalized);

    Ok(Json(ExplorerWithdrawalStatus {
        status: withdrawal_status(finalization.as_ref()),
        block_height: proof.block_height,
        batch_no,
        proof_available: finalized,
        withdrawal_root: proof.withdrawal_root,
        finalization: finalization.as_ref().map(finalization_status),
        leaf: proof.leaf,
    }))
}

pub(super) async fn get_withdrawal_proof(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let id = Hash32::from_hex(&id).map_err(|_| ApiError::bad_request("invalid withdrawal id"))?;
    let proof = state
        .storage
        .get_withdrawal_proof(id)
        .await?
        .ok_or_else(|| ApiError::not_found("withdrawal proof not found"))?;
    let batch_no = batch_no_from_block_height(proof.block_height)?;
    let finalized = state
        .storage
        .get_batch_finalization(batch_no)
        .await?
        .is_some_and(|record| record.status == BatchFinalizationStatus::Finalized);
    if !finalized {
        return Err(ApiError::conflict("withdrawal batch not finalized"));
    }
    Ok(Json(proof))
}

fn bounded_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

async fn list_start_height(
    storage: &DynStorage,
    before_height: Option<u64>,
) -> Result<Option<u64>, ApiError> {
    let Some(latest) = storage.latest_batch_commit(&[]).await? else {
        return Ok(None);
    };
    Ok(Some(before_height.map_or(latest.block_height, |before| {
        before.min(latest.block_height)
    })))
}

async fn scan_blocks_desc(
    storage: &DynStorage,
    start_height: u64,
    limit: usize,
) -> Result<Vec<L2Block>, ApiError> {
    let mut blocks = Vec::new();
    let mut height = Some(start_height);
    while let Some(current) = height {
        if let Some(block) = storage.get_block(current).await? {
            blocks.push(block);
            if blocks.len() >= limit {
                break;
            }
        }
        height = current.checked_sub(1);
    }
    Ok(blocks)
}

fn next_before_height(last_height: Option<u64>) -> Option<u64> {
    last_height.and_then(|height| height.checked_sub(1))
}

fn block_summary(block: &L2Block) -> ExplorerBlockSummary {
    ExplorerBlockSummary {
        height: block.header.height,
        block_hash: block.header.block_hash(),
        timestamp: block.header.timestamp,
        tx_count: block.transactions.len(),
        deposit_count: block
            .transactions
            .iter()
            .filter(|tx| matches!(tx.kind, L2TransactionKind::Deposit { .. }))
            .count(),
        withdrawal_count: block.withdrawals.len(),
        state_root: block.header.state_root,
        data_hash: block.header.data_hash,
        withdrawal_root: block.header.withdrawal_root,
    }
}

fn batch_status(record: &BatchCommitRecord) -> ExplorerBatchStatus {
    ExplorerBatchStatus {
        batch_no: record.batch_no,
        block_height: record.block_height,
        block_hash: record.block_hash,
        status: record.status.as_str(),
        message_hash_norm: record.message_hash_norm,
    }
}

fn finalization_status(record: &BatchFinalizationRecord) -> ExplorerFinalizationStatus {
    ExplorerFinalizationStatus {
        batch_no: record.batch_no,
        block_height: record.block_height,
        status: record.status.as_str(),
        finalize_after_unix: record.finalize_after_unix,
        message_hash_norm: record.message_hash_norm,
    }
}

fn withdrawal_status(finalization: Option<&BatchFinalizationRecord>) -> &'static str {
    match finalization.map(|record| record.status) {
        Some(BatchFinalizationStatus::Finalized) => "finalized",
        Some(BatchFinalizationStatus::Submitted) => "finalization_submitted",
        Some(BatchFinalizationStatus::Pending) => "pending_finalization",
        Some(BatchFinalizationStatus::Failed) => "finalization_failed",
        None => "waiting_for_finalization",
    }
}

fn batch_no_from_block_height(block_height: u64) -> Result<u64, ApiError> {
    block_height
        .checked_add(1)
        .ok_or_else(|| ApiError::bad_request("invalid withdrawal block height"))
}
