use async_trait::async_trait;
use l2_core::{Hash32, L2TransactionKind, SignedL2Transaction};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::error::MempoolError;

#[derive(Clone, Debug)]
pub struct ValidatedMempoolTx {
    pub(super) tx: SignedL2Transaction,
    pub(super) tx_hash: Hash32,
    pub(super) account_id: Hash32,
    pub(super) nonce: u64,
    pub(super) priority: MempoolTxPriority,
}

#[derive(Clone, Copy, Debug)]
pub struct MempoolStoreLimits {
    pub(super) replay_ttl: Duration,
    pub(super) nonce_lock_ttl: Duration,
    pub(super) max_global_queue: usize,
    pub(super) max_account_queue: usize,
    pub(super) max_account_nonce_window: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MempoolStoreStats {
    pub queued_global: usize,
    pub queued_accounts: usize,
    pub evicted: u64,
    pub replay_entries: Option<usize>,
    pub nonce_locks: Option<usize>,
    pub rate_windows: Option<usize>,
    pub leader_locked: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MempoolMetrics {
    pub accepted: u64,
    pub rejected: BTreeMap<String, u64>,
    pub store: MempoolStoreStats,
}

#[async_trait]
pub trait MempoolStore: Send + Sync {
    async fn consume_rate_limit(
        &self,
        account_id: Hash32,
        window: Duration,
        max_submissions: u32,
    ) -> Result<(), MempoolError>;

    async fn enqueue_validated(
        &self,
        tx: ValidatedMempoolTx,
        limits: MempoolStoreLimits,
    ) -> Result<(), MempoolError>;

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError>;
    async fn acquire_leader_lock(&self, owner: &str, ttl: Duration) -> Result<bool, MempoolError>;
    async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError>;
    async fn stats(&self) -> Result<MempoolStoreStats, MempoolError>;
}

pub type DynMempoolStore = Arc<dyn MempoolStore>;

#[derive(Debug, Default)]
pub(super) struct MempoolCounters {
    pub(super) accepted: u64,
    pub(super) rejected: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct MempoolTxPriority {
    pub gas_price: u128,
    pub max_fee: u128,
}

impl MempoolTxPriority {
    pub(super) fn from_tx(tx: &SignedL2Transaction) -> Self {
        let max_fee = u128::from(tx.gas_limit)
            .checked_mul(tx.max_gas_price)
            .unwrap_or(u128::MAX);
        Self {
            gas_price: tx.max_gas_price,
            max_fee,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MempoolPayloadClass {
    Transfer,
    Withdraw,
    CallContract,
    DeployContract,
    RotatePublicKey,
}

impl MempoolPayloadClass {
    pub(super) fn from_kind(kind: &L2TransactionKind) -> Option<Self> {
        match kind {
            L2TransactionKind::Transfer { .. } => Some(Self::Transfer),
            L2TransactionKind::Withdraw { .. } => Some(Self::Withdraw),
            L2TransactionKind::CallContract { .. } => Some(Self::CallContract),
            L2TransactionKind::DeployContract { .. } => Some(Self::DeployContract),
            L2TransactionKind::RotatePublicKey { .. } => Some(Self::RotatePublicKey),
            L2TransactionKind::Deposit { .. } | L2TransactionKind::InternalMessage { .. } => None,
        }
    }

    pub(super) fn limit_name(self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::Withdraw => "withdraw",
            Self::CallContract => "call",
            Self::DeployContract => "deploy",
            Self::RotatePublicKey => "rotate_public_key",
        }
    }

    pub(super) fn reason_code(self) -> &'static str {
        match self {
            Self::Transfer => "transfer_payload_too_large",
            Self::Withdraw => "withdraw_payload_too_large",
            Self::CallContract => "call_payload_too_large",
            Self::DeployContract => "deploy_payload_too_large",
            Self::RotatePublicKey => "rotate_public_key_payload_too_large",
        }
    }
}

impl fmt::Display for MempoolPayloadClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.limit_name())
    }
}

#[derive(Clone, Debug)]
pub(super) struct QueuedMempoolTx {
    pub account_id: Hash32,
    pub nonce: u64,
    pub tx_hash: Hash32,
    pub priority: MempoolTxPriority,
    pub sequence: u64,
    pub tx: SignedL2Transaction,
}

impl QueuedMempoolTx {
    pub(super) fn from_validated(validated: ValidatedMempoolTx, sequence: u64) -> Self {
        Self {
            account_id: validated.account_id,
            nonce: validated.nonce,
            tx_hash: validated.tx_hash,
            priority: validated.priority,
            sequence,
            tx: validated.tx,
        }
    }

    pub(super) fn from_tx(tx: SignedL2Transaction, sequence: u64) -> Self {
        let account_id = tx.from.unwrap_or_default();
        let nonce = tx.nonce;
        let tx_hash = tx.tx_hash();
        let priority = MempoolTxPriority::from_tx(&tx);
        Self {
            account_id,
            nonce,
            tx_hash,
            priority,
            sequence,
            tx,
        }
    }
}

pub(super) fn fair_order_indices(queue: &[QueuedMempoolTx], max_txs: usize) -> Vec<usize> {
    if max_txs == 0 || queue.is_empty() {
        return vec![];
    }

    let mut by_account: BTreeMap<Hash32, Vec<usize>> = BTreeMap::new();
    for (index, queued) in queue.iter().enumerate() {
        by_account.entry(queued.account_id).or_default().push(index);
    }
    for indices in by_account.values_mut() {
        indices.sort_by(|left, right| {
            queue[*left]
                .nonce
                .cmp(&queue[*right].nonce)
                .then_with(|| queue[*left].sequence.cmp(&queue[*right].sequence))
                .then_with(|| queue[*left].tx_hash.cmp(&queue[*right].tx_hash))
        });
    }

    let mut selected = Vec::with_capacity(max_txs.min(queue.len()));
    while selected.len() < max_txs {
        let mut candidates = by_account
            .iter()
            .filter_map(|(account_id, indices)| indices.first().map(|index| (*account_id, *index)))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            break;
        }
        candidates.sort_by(|(left_account, left_index), (right_account, right_index)| {
            queue[*right_index]
                .priority
                .cmp(&queue[*left_index].priority)
                .then_with(|| left_account.cmp(right_account))
                .then_with(|| queue[*left_index].tx_hash.cmp(&queue[*right_index].tx_hash))
        });

        for (account_id, index) in candidates {
            if selected.len() >= max_txs {
                break;
            }
            let Some(indices) = by_account.get_mut(&account_id) else {
                continue;
            };
            if indices.first().copied() != Some(index) {
                continue;
            }
            indices.remove(0);
            selected.push(index);
        }
    }

    selected
}
