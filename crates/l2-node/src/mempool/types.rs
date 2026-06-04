use async_trait::async_trait;
use l2_core::{Hash32, SignedL2Transaction};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::error::MempoolError;

#[derive(Clone, Debug)]
pub struct ValidatedMempoolTx {
    pub(super) tx: SignedL2Transaction,
    pub(super) tx_hash: Hash32,
    pub(super) account_id: Hash32,
    pub(super) nonce: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct MempoolStoreLimits {
    pub(super) replay_ttl: Duration,
    pub(super) nonce_lock_ttl: Duration,
    pub(super) max_global_queue: usize,
    pub(super) max_account_queue: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MempoolStoreStats {
    pub queued_global: usize,
    pub queued_accounts: usize,
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
