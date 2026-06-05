mod flow;

use super::super::super::{ApiError, AppState};
use super::super::bounded_limit;
use crate::storage::StoredTransaction;
use axum::extract::{Path, Query, State};
use axum::Json;
use flow::{transaction_flow, ExplorerTransactionFlowNode};
use l2_core::{
    decode_contract_cell_boc_base64, interface_for_code_hash, l2_raw_address,
    l2_user_friendly_address, Hash32, L2TransactionKind, Receipt, ReceiptStatus,
    SignedL2Transaction, DEFAULT_MAX_TVM_BOC_BYTES, ENWALLET_V5R1_CODE_HASH,
    ENWALLET_V5R1_INTERFACE, ENWALLET_V5R1_LABEL,
};
use serde::{Deserialize, Serialize};

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
    pub(in crate::api) interface: Option<&'static str>,
    pub(in crate::api) interface_label: Option<&'static str>,
    pub(in crate::api) operation: Option<&'static str>,
    pub(in crate::api) direction: &'static str,
    pub(in crate::api) participants: Vec<ExplorerParticipant>,
    pub(in crate::api) asset_id: Option<u32>,
    pub(in crate::api) amount: Option<String>,
    pub(in crate::api) status: &'static str,
    pub(in crate::api) gas_charged: Option<String>,
    pub(in crate::api) reason: Option<String>,
    pub(in crate::api) withdrawal_id: Option<Hash32>,
    pub(in crate::api) event_count: usize,
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
    pub(in crate::api) flow: Vec<ExplorerTransactionFlowNode>,
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

pub(in crate::api) async fn explorer_account_transactions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ExplorerAccountTransactionsQuery>,
) -> Result<Json<ExplorerAccountTransactions>, ApiError> {
    let id =
        l2_core::parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
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
        flow: transaction_flow(&record),
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
    let event_count = record
        .receipt
        .as_ref()
        .map_or(0, |receipt| receipt.events.len());
    ExplorerTransactionSummary {
        block_height: record.block_height,
        tx_index: record.tx_index,
        timestamp: record.block_timestamp,
        block_hash: record.block_hash,
        tx_hash,
        kind: kind_name(&record.transaction.kind),
        interface: transaction_interface(&record.transaction.kind).map(|(id, _)| id),
        interface_label: transaction_interface(&record.transaction.kind).map(|(_, label)| label),
        operation: transaction_operation(&record.transaction.kind),
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
        event_count,
    }
}

fn transaction_interface(kind: &L2TransactionKind) -> Option<(&'static str, &'static str)> {
    match kind {
        L2TransactionKind::DeployContract {
            code_boc_base64, ..
        } => deploy_code_hash(code_boc_base64).and_then(interface_for_code_hash),
        L2TransactionKind::CallContract {
            body_boc_base64, ..
        } => {
            if wallet_signed_operation(body_boc_base64).is_some() {
                Some((ENWALLET_V5R1_INTERFACE, ENWALLET_V5R1_LABEL))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn transaction_operation(kind: &L2TransactionKind) -> Option<&'static str> {
    match kind {
        L2TransactionKind::DeployContract {
            code_boc_base64, ..
        } if deploy_code_hash(code_boc_base64) == Some(ENWALLET_V5R1_CODE_HASH) => {
            Some("wallet_init")
        }
        L2TransactionKind::CallContract {
            body_boc_base64, ..
        } => wallet_signed_operation(body_boc_base64),
        _ => None,
    }
}

fn deploy_code_hash(code_boc_base64: &str) -> Option<Hash32> {
    decode_contract_cell_boc_base64(code_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
        .ok()
        .map(|cell| cell.cell_hash)
}

fn wallet_signed_operation(body_boc_base64: &str) -> Option<&'static str> {
    let body = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        body_boc_base64.as_bytes(),
    )
    .ok()?;
    let root = tonlib_core::cell::BagOfCells::parse(&body)
        .and_then(tonlib_core::cell::BagOfCells::single_root)
        .ok()?;
    let mut parser = root.parser();
    let opcode = parser.load_u32(32).ok()?;
    match opcode {
        0x7369_676e => Some("wallet_signed_external"),
        0x7369_6e74 => Some("wallet_signed_internal"),
        _ => None,
    }
}

fn kind_name(kind: &L2TransactionKind) -> &'static str {
    match kind {
        L2TransactionKind::Deposit { .. } => "deposit",
        L2TransactionKind::Transfer { .. } => "transfer",
        L2TransactionKind::Withdraw { .. } => "withdraw",
        L2TransactionKind::DeployContract { .. } => "deploy_contract",
        L2TransactionKind::CallContract { .. } => "call_contract",
        L2TransactionKind::InternalMessage { .. } => "internal_message",
        L2TransactionKind::RotatePublicKey { .. } => "rotate_public_key",
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
        L2TransactionKind::InternalMessage { to, .. } => Some(*to),
        L2TransactionKind::RotatePublicKey { .. } => None,
        L2TransactionKind::Withdraw { .. } => None,
    }
}

fn asset_id(kind: &L2TransactionKind) -> Option<u32> {
    match kind {
        L2TransactionKind::Deposit { asset_id, .. }
        | L2TransactionKind::Transfer { asset_id, .. }
        | L2TransactionKind::Withdraw { asset_id, .. } => Some(*asset_id),
        L2TransactionKind::DeployContract { .. }
        | L2TransactionKind::CallContract { .. }
        | L2TransactionKind::InternalMessage { .. }
        | L2TransactionKind::RotatePublicKey { .. } => None,
    }
}

fn amount(kind: &L2TransactionKind) -> Option<u128> {
    match kind {
        L2TransactionKind::Deposit { amount, .. }
        | L2TransactionKind::Transfer { amount, .. }
        | L2TransactionKind::Withdraw { amount, .. } => Some(*amount),
        L2TransactionKind::DeployContract { .. }
        | L2TransactionKind::CallContract { .. }
        | L2TransactionKind::InternalMessage { .. }
        | L2TransactionKind::RotatePublicKey { .. } => None,
    }
}

pub(super) fn receipt_status(receipt: Option<&Receipt>) -> &'static str {
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
        L2TransactionKind::InternalMessage { from, to, .. } => {
            out.push(participant("internal_from", *from));
            out.push(participant("internal_to", *to));
        }
        L2TransactionKind::RotatePublicKey { .. } => {}
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
