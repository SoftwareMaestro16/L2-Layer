use super::super::{ApiError, AppState};
use super::bounded_limit;
use crate::storage::StoredTransaction;
use axum::extract::{Path, Query, State};
use axum::Json;
use l2_core::{
    l2_raw_address, l2_user_friendly_address, parse_l2_address, Hash32, L2TransactionKind, Receipt,
    ReceiptStatus, SignedL2Transaction,
};
use serde::{Deserialize, Serialize};

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
    pub(in crate::api) last_lt: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerBalance {
    pub(in crate::api) asset_id: u32,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(in crate::api) amount: u128,
}

#[derive(Clone, Debug, Deserialize)]
pub(in crate::api) struct ExplorerAccountTransactionsQuery {
    pub(in crate::api) limit: Option<usize>,
    pub(in crate::api) before_height: Option<u64>,
    pub(in crate::api) before_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerAccountTransactions {
    pub(in crate::api) items: Vec<ExplorerTransactionSummary>,
    pub(in crate::api) next_cursor: Option<ExplorerTransactionCursor>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerTransactionCursor {
    pub(in crate::api) before_height: u64,
    pub(in crate::api) before_index: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerTransactionSummary {
    pub(in crate::api) block_height: u64,
    pub(in crate::api) tx_index: usize,
    pub(in crate::api) timestamp: u64,
    pub(in crate::api) block_hash: Hash32,
    pub(in crate::api) tx_hash: Hash32,
    pub(in crate::api) kind: &'static str,
    pub(in crate::api) direction: &'static str,
    pub(in crate::api) participants: Vec<ExplorerParticipant>,
    pub(in crate::api) asset_id: Option<u32>,
    pub(in crate::api) amount: Option<String>,
    pub(in crate::api) status: &'static str,
    pub(in crate::api) gas_charged: Option<String>,
    pub(in crate::api) reason: Option<String>,
    pub(in crate::api) withdrawal_id: Option<Hash32>,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerParticipant {
    pub(in crate::api) role: &'static str,
    pub(in crate::api) account_id: Hash32,
    pub(in crate::api) raw_address: String,
    pub(in crate::api) user_friendly_address: String,
}

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerTransactionDetail {
    #[serde(flatten)]
    pub(in crate::api) summary: ExplorerTransactionSummary,
    pub(in crate::api) chain_id: String,
    pub(in crate::api) nonce: u64,
    pub(in crate::api) gas_limit: u64,
    #[serde(with = "l2_core::serde_u128_string")]
    pub(in crate::api) max_gas_price: u128,
    pub(in crate::api) tx_root: Hash32,
    pub(in crate::api) receipt_root: Hash32,
    pub(in crate::api) withdrawal_root: Hash32,
    pub(in crate::api) data_hash: Hash32,
    pub(in crate::api) state_root: Hash32,
    pub(in crate::api) raw_transaction: SignedL2Transaction,
    pub(in crate::api) raw_receipt: Option<Receipt>,
}

pub(in crate::api) async fn explorer_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ExplorerAccount>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
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
        last_lt: account.last_lt,
    }))
}

pub(in crate::api) async fn explorer_account_transactions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ExplorerAccountTransactionsQuery>,
) -> Result<Json<ExplorerAccountTransactions>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    if query.before_index.is_some() && query.before_height.is_none() {
        return Err(ApiError::bad_request(
            "before_height required with before_index",
        ));
    }
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .list_account_transactions(id, query.before_height, query.before_index, limit)
        .await?;
    let next_cursor = if records.len() == limit {
        records.last().map(|record| ExplorerTransactionCursor {
            before_height: record.block_height,
            before_index: record.tx_index,
        })
    } else {
        None
    };

    Ok(Json(ExplorerAccountTransactions {
        items: records
            .into_iter()
            .map(|record| transaction_summary(record, Some(id)))
            .collect(),
        next_cursor,
    }))
}

pub(in crate::api) async fn explorer_tx(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<ExplorerTransactionDetail>, ApiError> {
    let hash = Hash32::from_hex(&hash).map_err(|_| ApiError::bad_request("invalid tx hash"))?;
    let record = state
        .storage
        .get_transaction(hash)
        .await?
        .ok_or_else(|| ApiError::not_found("transaction not found"))?;
    let block = state
        .storage
        .get_block(record.block_height)
        .await?
        .ok_or_else(|| ApiError::not_found("block not found"))?;
    let summary = transaction_summary(record.clone(), None);

    Ok(Json(ExplorerTransactionDetail {
        summary,
        chain_id: record.transaction.chain_id.clone(),
        nonce: record.transaction.nonce,
        gas_limit: record.transaction.gas_limit,
        max_gas_price: record.transaction.max_gas_price,
        tx_root: block.header.tx_root,
        receipt_root: block.header.receipt_root,
        withdrawal_root: block.header.withdrawal_root,
        data_hash: block.header.data_hash,
        state_root: block.header.state_root,
        raw_transaction: record.transaction,
        raw_receipt: record.receipt,
    }))
}

fn transaction_summary(
    record: StoredTransaction,
    account_filter: Option<Hash32>,
) -> ExplorerTransactionSummary {
    let tx_hash = record.transaction.tx_hash();
    ExplorerTransactionSummary {
        block_height: record.block_height,
        tx_index: record.tx_index,
        timestamp: record.block_timestamp,
        block_hash: record.block_hash,
        tx_hash,
        kind: kind_name(&record.transaction.kind),
        direction: direction(&record.transaction, account_filter),
        participants: participants(&record.transaction),
        asset_id: asset_id(&record.transaction.kind),
        amount: amount(&record.transaction.kind).map(|amount| amount.to_string()),
        status: receipt_status(record.receipt.as_ref()),
        gas_charged: record
            .receipt
            .as_ref()
            .map(|receipt| receipt.gas_charged.to_string()),
        reason: record
            .receipt
            .as_ref()
            .and_then(|receipt| receipt.reason.clone()),
        withdrawal_id: record.receipt.and_then(|receipt| receipt.withdrawal_id),
    }
}

fn kind_name(kind: &L2TransactionKind) -> &'static str {
    match kind {
        L2TransactionKind::Deposit { .. } => "deposit",
        L2TransactionKind::Transfer { .. } => "transfer",
        L2TransactionKind::Withdraw { .. } => "withdraw",
        L2TransactionKind::DeployContract { .. } => "deploy_contract",
        L2TransactionKind::CallContract { .. } => "call_contract",
    }
}

fn direction(tx: &SignedL2Transaction, account_filter: Option<Hash32>) -> &'static str {
    let Some(account_id) = account_filter else {
        return "n/a";
    };
    if tx.from == Some(account_id) && recipient(tx) == Some(account_id) {
        return "self";
    }
    if tx.from == Some(account_id) {
        return "out";
    }
    if recipient(tx) == Some(account_id) {
        return "in";
    }
    "related"
}

fn recipient(tx: &SignedL2Transaction) -> Option<Hash32> {
    match &tx.kind {
        L2TransactionKind::Deposit { recipient, .. } => Some(*recipient),
        L2TransactionKind::Transfer { to, .. } => Some(*to),
        L2TransactionKind::DeployContract { contract, .. } => Some(*contract),
        L2TransactionKind::CallContract { contract, .. } => Some(*contract),
        L2TransactionKind::Withdraw { .. } => None,
    }
}

fn asset_id(kind: &L2TransactionKind) -> Option<u32> {
    match kind {
        L2TransactionKind::Deposit { asset_id, .. }
        | L2TransactionKind::Transfer { asset_id, .. }
        | L2TransactionKind::Withdraw { asset_id, .. } => Some(*asset_id),
        L2TransactionKind::DeployContract { .. } | L2TransactionKind::CallContract { .. } => None,
    }
}

fn amount(kind: &L2TransactionKind) -> Option<u128> {
    match kind {
        L2TransactionKind::Deposit { amount, .. }
        | L2TransactionKind::Transfer { amount, .. }
        | L2TransactionKind::Withdraw { amount, .. } => Some(*amount),
        L2TransactionKind::DeployContract { .. } | L2TransactionKind::CallContract { .. } => None,
    }
}

fn receipt_status(receipt: Option<&Receipt>) -> &'static str {
    match receipt.map(|receipt| &receipt.status) {
        Some(ReceiptStatus::Applied) => "applied",
        Some(ReceiptStatus::Rejected) => "rejected",
        None => "unknown",
    }
}

fn participants(tx: &SignedL2Transaction) -> Vec<ExplorerParticipant> {
    let mut out = Vec::new();
    if let Some(from) = tx.from {
        out.push(participant("from", from));
    }
    match &tx.kind {
        L2TransactionKind::Deposit { recipient, .. } => {
            out.push(participant("recipient", *recipient))
        }
        L2TransactionKind::Transfer { to, .. } => out.push(participant("to", *to)),
        L2TransactionKind::DeployContract { contract, .. }
        | L2TransactionKind::CallContract { contract, .. } => {
            out.push(participant("contract", *contract))
        }
        L2TransactionKind::Withdraw { .. } => {}
    }
    out
}

fn participant(role: &'static str, account_id: Hash32) -> ExplorerParticipant {
    ExplorerParticipant {
        role,
        account_id,
        raw_address: l2_raw_address(account_id),
        user_friendly_address: l2_user_friendly_address(account_id),
    }
}
