use async_trait::async_trait;
use l2_core::{Hash32, SignedL2Transaction};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::error::MempoolError;
use super::types::{MempoolStore, MempoolStoreLimits, MempoolStoreStats, ValidatedMempoolTx};

#[derive(Debug, Default)]
pub struct MemoryMempoolStore {
    state: Mutex<MemoryMempoolState>,
}

#[derive(Debug, Default)]
struct MemoryMempoolState {
    queue: VecDeque<SignedL2Transaction>,
    queued_hashes: BTreeSet<Hash32>,
    account_queue_counts: BTreeMap<Hash32, usize>,
    replay: BTreeMap<Hash32, Instant>,
    nonce_locks: BTreeMap<(Hash32, u64), Instant>,
    rate_windows: BTreeMap<Hash32, VecDeque<Instant>>,
    leader_lock: Option<MemoryLeaderLock>,
}

#[derive(Debug)]
struct MemoryLeaderLock {
    owner: String,
    expires_at: Instant,
}

#[async_trait]
impl MempoolStore for MemoryMempoolStore {
    async fn consume_rate_limit(
        &self,
        account_id: Hash32,
        window: Duration,
        max_submissions: u32,
    ) -> Result<(), MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();
        let now = Instant::now();
        let entries = state.rate_windows.entry(account_id).or_default();
        entries.retain(|timestamp| *timestamp + window > now);
        if entries.len() >= max_submissions as usize {
            return Err(MempoolError::RateLimited { account_id });
        }
        entries.push_back(now);
        Ok(())
    }

    async fn enqueue_validated(
        &self,
        validated: ValidatedMempoolTx,
        limits: MempoolStoreLimits,
    ) -> Result<(), MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();

        let tx_hash = validated.tx_hash;
        let account_id = validated.account_id;
        let nonce = validated.nonce;
        if state.replay.contains_key(&tx_hash) || state.queued_hashes.contains(&tx_hash) {
            return Err(MempoolError::DuplicateTx(tx_hash));
        }
        if state.nonce_locks.contains_key(&(account_id, nonce)) {
            return Err(MempoolError::NonceLocked { account_id, nonce });
        }
        if state.queue.len() >= limits.max_global_queue {
            return Err(MempoolError::GlobalQueueFull);
        }
        if state
            .account_queue_counts
            .get(&account_id)
            .copied()
            .unwrap_or_default()
            >= limits.max_account_queue
        {
            return Err(MempoolError::AccountQueueFull { account_id });
        }

        let now = Instant::now();
        state.replay.insert(tx_hash, now + limits.replay_ttl);
        state
            .nonce_locks
            .insert((account_id, nonce), now + limits.nonce_lock_ttl);
        state.queued_hashes.insert(tx_hash);
        *state.account_queue_counts.entry(account_id).or_default() += 1;
        state.queue.push_back(validated.tx);
        Ok(())
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        let mut state = self.state.lock().await;
        let mut txs = Vec::with_capacity(max_txs.min(state.queue.len()));
        for _ in 0..max_txs {
            let Some(tx) = state.queue.pop_front() else {
                break;
            };
            state.queued_hashes.remove(&tx.tx_hash());
            if let Some(account_id) = tx.from {
                if let Some(count) = state.account_queue_counts.get_mut(&account_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        state.account_queue_counts.remove(&account_id);
                    }
                }
            }
            txs.push(tx);
        }
        Ok(txs)
    }

    async fn acquire_leader_lock(&self, owner: &str, ttl: Duration) -> Result<bool, MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();
        if state.leader_lock.is_some() {
            return Ok(false);
        }
        state.leader_lock = Some(MemoryLeaderLock {
            owner: owner.to_owned(),
            expires_at: Instant::now() + ttl,
        });
        Ok(true)
    }

    async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        let mut state = self.state.lock().await;
        let Some(lock) = state.leader_lock.as_ref() else {
            return Ok(false);
        };
        if lock.owner != owner {
            return Ok(false);
        }
        state.leader_lock = None;
        Ok(true)
    }

    async fn stats(&self) -> Result<MempoolStoreStats, MempoolError> {
        let mut state = self.state.lock().await;
        state.cleanup_expired();
        Ok(MempoolStoreStats {
            queued_global: state.queue.len(),
            queued_accounts: state.account_queue_counts.len(),
            replay_entries: Some(state.replay.len()),
            nonce_locks: Some(state.nonce_locks.len()),
            rate_windows: Some(state.rate_windows.len()),
            leader_locked: state.leader_lock.is_some(),
        })
    }
}

impl MemoryMempoolState {
    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.replay.retain(|_, expires_at| *expires_at > now);
        self.nonce_locks.retain(|_, expires_at| *expires_at > now);
        self.rate_windows.retain(|_, entries| !entries.is_empty());
        if self
            .leader_lock
            .as_ref()
            .is_some_and(|lock| lock.expires_at <= now)
        {
            self.leader_lock = None;
        }
    }
}
