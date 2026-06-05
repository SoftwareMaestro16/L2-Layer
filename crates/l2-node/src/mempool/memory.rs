use async_trait::async_trait;
use l2_core::{Hash32, SignedL2Transaction};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::error::MempoolError;
use super::types::{
    fair_order_indices, MempoolStore, MempoolStoreLimits, MempoolStoreStats, QueuedMempoolTx,
    ValidatedMempoolTx,
};

#[derive(Debug, Default)]
pub struct MemoryMempoolStore {
    state: Mutex<MemoryMempoolState>,
}

#[derive(Debug, Default)]
struct MemoryMempoolState {
    queue: VecDeque<QueuedMempoolTx>,
    queued_hashes: BTreeSet<Hash32>,
    account_queue_counts: BTreeMap<Hash32, usize>,
    account_pending_nonces: BTreeMap<Hash32, BTreeSet<u64>>,
    replay: BTreeMap<Hash32, Instant>,
    nonce_locks: BTreeMap<(Hash32, u64), Instant>,
    rate_windows: BTreeMap<Hash32, VecDeque<Instant>>,
    leader_lock: Option<MemoryLeaderLock>,
    next_sequence: u64,
    evicted: u64,
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
        if state
            .account_queue_counts
            .get(&account_id)
            .copied()
            .unwrap_or_default()
            >= limits.max_account_queue
        {
            return Err(MempoolError::AccountQueueFull { account_id });
        }
        state.validate_pending_nonce_window(account_id, nonce, limits.max_account_nonce_window)?;
        if state.queue.len() >= limits.max_global_queue && !state.evict_for(&validated) {
            return Err(MempoolError::GlobalQueueFull);
        }

        let now = Instant::now();
        state.replay.insert(tx_hash, now + limits.replay_ttl);
        state
            .nonce_locks
            .insert((account_id, nonce), now + limits.nonce_lock_ttl);
        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.wrapping_add(1);
        let queued = QueuedMempoolTx::from_validated(validated, sequence);
        state.record_queued_metadata(&queued);
        state.queue.push_back(queued);
        Ok(())
    }

    async fn pop_batch(&self, max_txs: usize) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        let mut state = self.state.lock().await;
        let snapshot = state.queue.iter().cloned().collect::<Vec<_>>();
        let selected_indices = fair_order_indices(&snapshot, max_txs)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut txs = Vec::with_capacity(selected_indices.len());
        let mut retained = VecDeque::with_capacity(state.queue.len().saturating_sub(txs.len()));
        let mut index = 0usize;
        while let Some(queued) = state.queue.pop_front() {
            if selected_indices.contains(&index) {
                state.remove_queued_metadata(&queued);
                txs.push(queued.tx);
            } else {
                retained.push_back(queued);
            }
            index += 1;
        }
        state.queue = retained;
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
            evicted: state.evicted,
            replay_entries: Some(state.replay.len()),
            nonce_locks: Some(state.nonce_locks.len()),
            rate_windows: Some(state.rate_windows.len()),
            leader_locked: state.leader_lock.is_some(),
        })
    }
}

impl MemoryMempoolState {
    fn validate_pending_nonce_window(
        &self,
        account_id: Hash32,
        nonce: u64,
        window: u64,
    ) -> Result<(), MempoolError> {
        let Some(nonces) = self.account_pending_nonces.get(&account_id) else {
            return Ok(());
        };
        if nonces.contains(&nonce) {
            return Err(MempoolError::NonceLocked { account_id, nonce });
        }
        let Some(min_nonce) = nonces.iter().next().copied() else {
            return Ok(());
        };
        let Some(max_nonce) = nonces.iter().next_back().copied() else {
            return Ok(());
        };
        let candidate_min = min_nonce.min(nonce);
        let candidate_max = max_nonce.max(nonce);
        if candidate_max.saturating_sub(candidate_min) > window {
            return Err(MempoolError::AccountNonceWindowExceeded {
                account_id,
                nonce,
                min_nonce: candidate_min,
                max_nonce: candidate_max,
                window,
            });
        }
        Ok(())
    }

    fn evict_for(&mut self, incoming: &ValidatedMempoolTx) -> bool {
        let Some((worst_index, worst_priority)) = self
            .queue
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| right.sequence.cmp(&left.sequence))
                    .then_with(|| left.tx_hash.cmp(&right.tx_hash))
            })
            .map(|(index, queued)| (index, queued.priority))
        else {
            return false;
        };
        if incoming.priority <= worst_priority {
            return false;
        }
        let Some(evicted) = self.queue.remove(worst_index) else {
            return false;
        };
        self.remove_queued_metadata(&evicted);
        self.evicted += 1;
        true
    }

    fn record_queued_metadata(&mut self, queued: &QueuedMempoolTx) {
        self.queued_hashes.insert(queued.tx_hash);
        *self
            .account_queue_counts
            .entry(queued.account_id)
            .or_default() += 1;
        self.account_pending_nonces
            .entry(queued.account_id)
            .or_default()
            .insert(queued.nonce);
    }

    fn remove_queued_metadata(&mut self, queued: &QueuedMempoolTx) {
        self.queued_hashes.remove(&queued.tx_hash);
        if let Some(count) = self.account_queue_counts.get_mut(&queued.account_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.account_queue_counts.remove(&queued.account_id);
            }
        }
        if let Some(nonces) = self.account_pending_nonces.get_mut(&queued.account_id) {
            nonces.remove(&queued.nonce);
            if nonces.is_empty() {
                self.account_pending_nonces.remove(&queued.account_id);
            }
        }
    }

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
