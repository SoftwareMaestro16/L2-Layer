use super::receipt_status;
use crate::storage::StoredTransaction;
use l2_core::{l2_raw_address, l2_user_friendly_address, Hash32, L2TransactionKind};
use serde::Serialize;
use serde_json::json;

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct ExplorerTransactionFlowNode {
    pub(in crate::api) id: String,
    pub(in crate::api) label: String,
    pub(in crate::api) role: String,
    pub(in crate::api) account_id: Option<Hash32>,
    pub(in crate::api) raw_address: Option<String>,
    pub(in crate::api) user_friendly_address: Option<String>,
    pub(in crate::api) asset_id: Option<u32>,
    pub(in crate::api) amount: Option<String>,
    pub(in crate::api) gas_charged: Option<String>,
    pub(in crate::api) status: Option<&'static str>,
    pub(in crate::api) reason: Option<String>,
    pub(in crate::api) details: serde_json::Value,
}

pub(in crate::api) fn transaction_flow(
    record: &StoredTransaction,
) -> Vec<ExplorerTransactionFlowNode> {
    let mut nodes = Vec::new();
    if let Some(from) = record.transaction.from {
        nodes.push(account_flow_node("from", "Sender", "from", from));
    }
    match &record.transaction.kind {
        L2TransactionKind::Deposit {
            deposit_id,
            asset_id,
            recipient,
            amount,
        } => {
            nodes.push(system_flow_node(
                "deposit",
                "Deposit",
                "deposit",
                Some(*asset_id),
                Some(*amount),
                json!({ "deposit_id": deposit_id }),
            ));
            nodes.push(account_flow_node(
                "recipient",
                "Recipient",
                "recipient",
                *recipient,
            ));
        }
        L2TransactionKind::Transfer {
            to,
            asset_id,
            amount,
        } => {
            nodes.push(value_flow_node("transfer", "Transfer", *asset_id, *amount));
            nodes.push(account_flow_node("to", "Recipient", "to", *to));
        }
        L2TransactionKind::Withdraw {
            asset_id,
            amount,
            l1_recipient,
        } => nodes.push(system_flow_node(
            "withdraw",
            "Withdrawal",
            "withdrawal",
            Some(*asset_id),
            Some(*amount),
            json!({ "l1_recipient": l1_recipient }),
        )),
        L2TransactionKind::DeployContract { contract, .. } => {
            nodes.push(account_flow_node(
                "contract", "Contract", "contract", *contract,
            ));
            nodes.push(system_flow_node(
                "deploy",
                "Deploy",
                "contract_deploy",
                None,
                None,
                json!({ "contract": contract }),
            ));
        }
        L2TransactionKind::CallContract { contract, .. } => {
            nodes.push(account_flow_node(
                "contract", "Contract", "contract", *contract,
            ));
            nodes.push(system_flow_node(
                "call",
                "Call",
                "contract_call",
                None,
                None,
                json!({ "contract": contract }),
            ));
        }
        L2TransactionKind::InternalMessage {
            message_id,
            from,
            to,
            value,
            bounce,
            bounced,
            ..
        } => {
            nodes.push(account_flow_node(
                "internal_from",
                "Internal from",
                "internal_from",
                *from,
            ));
            nodes.push(system_flow_node(
                "internal_message",
                "Internal message",
                "internal_message",
                Some(0),
                Some(*value),
                json!({ "message_id": message_id, "bounce": bounce, "bounced": bounced }),
            ));
            nodes.push(account_flow_node(
                "internal_to",
                "Internal to",
                "internal_to",
                *to,
            ));
        }
        L2TransactionKind::RotatePublicKey { new_public_key } => nodes.push(system_flow_node(
            "rotate_public_key",
            "Rotate public key",
            "account_security",
            None,
            None,
            json!({ "new_public_key": new_public_key }),
        )),
    }
    if let Some(receipt) = &record.receipt {
        nodes.push(ExplorerTransactionFlowNode {
            id: "receipt".to_owned(),
            label: "Receipt".to_owned(),
            role: "receipt".to_owned(),
            account_id: None,
            raw_address: None,
            user_friendly_address: None,
            asset_id: None,
            amount: None,
            gas_charged: Some(receipt.gas_charged.to_string()),
            status: Some(receipt_status(Some(receipt))),
            reason: receipt.reason.clone(),
            details: json!({
                "withdrawal_id": receipt.withdrawal_id,
                "event_count": receipt.events.len(),
                "events": receipt.events,
            }),
        });
    }
    nodes
}

fn account_flow_node(
    id: &str,
    label: &str,
    role: &str,
    account_id: Hash32,
) -> ExplorerTransactionFlowNode {
    ExplorerTransactionFlowNode {
        id: id.to_owned(),
        label: label.to_owned(),
        role: role.to_owned(),
        account_id: Some(account_id),
        raw_address: Some(l2_raw_address(account_id)),
        user_friendly_address: Some(l2_user_friendly_address(account_id)),
        asset_id: None,
        amount: None,
        gas_charged: None,
        status: None,
        reason: None,
        details: json!({ "account_id": account_id }),
    }
}

fn value_flow_node(
    id: &str,
    label: &str,
    asset_id: u32,
    amount: u128,
) -> ExplorerTransactionFlowNode {
    system_flow_node(id, label, "value", Some(asset_id), Some(amount), json!({}))
}

fn system_flow_node(
    id: &str,
    label: &str,
    role: &str,
    asset_id: Option<u32>,
    amount: Option<u128>,
    details: serde_json::Value,
) -> ExplorerTransactionFlowNode {
    ExplorerTransactionFlowNode {
        id: id.to_owned(),
        label: label.to_owned(),
        role: role.to_owned(),
        account_id: None,
        raw_address: None,
        user_friendly_address: None,
        asset_id,
        amount: amount.map(|value| value.to_string()),
        gas_charged: None,
        status: None,
        reason: None,
        details,
    }
}
