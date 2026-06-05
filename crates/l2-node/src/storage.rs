use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, Receipt, SignedL2Transaction, WithdrawalProof};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::config::NodeConfig;

mod da_payload;
mod observer;
mod postgres;
mod postgres_da;
mod postgres_finalization;
mod postgres_observer;
mod postgres_util;

pub use observer::ObserverCheckpoint;
pub use postgres::PostgresStorage;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub block_height: u64,
    pub block_timestamp: u64,
    pub block_hash: Hash32,
    pub tx_index: usize,
    pub transaction: SignedL2Transaction,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct L1Cursor {
    pub lt: u64,
    pub hash: Hash32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchCommitStatus {
    Pending,
    Submitted,
    Confirmed,
    Failed,
}

impl BatchCommitStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "submitted" => Some(Self::Submitted),
            "confirmed" => Some(Self::Confirmed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchCommitRecord {
    pub batch_no: u64,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub status: BatchCommitStatus,
    pub attempts: u32,
    pub message_hash: Option<Hash32>,
    pub message_hash_norm: Option<Hash32>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchFinalizationStatus {
    Pending,
    Submitted,
    Finalized,
    Failed,
}

impl BatchFinalizationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Submitted => "submitted",
            Self::Finalized => "finalized",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "submitted" => Some(Self::Submitted),
            "finalized" => Some(Self::Finalized),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchFinalizationRecord {
    pub batch_no: u64,
    pub block_height: u64,
    pub status: BatchFinalizationStatus,
    pub attempts: u32,
    pub finalize_after_unix: u64,
    pub message_hash: Option<Hash32>,
    pub message_hash_norm: Option<Hash32>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredBatchPayload {
    pub block_height: u64,
    pub block_hash: Hash32,
    pub data_hash: Hash32,
    pub payload_bytes: Vec<u8>,
    pub public_ref: Option<String>,
    pub public_uri: Option<String>,
}

impl BatchCommitRecord {
    pub fn pending(block: &L2Block) -> Option<Self> {
        Some(Self {
            batch_no: block.header.height.checked_add(1)?,
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            status: BatchCommitStatus::Pending,
            attempts: 0,
            message_hash: None,
            message_hash_norm: None,
            last_error: None,
        })
    }
}

impl BatchFinalizationRecord {
    pub fn pending(commit: &BatchCommitRecord, finalize_after_unix: u64) -> Self {
        Self {
            batch_no: commit.batch_no,
            block_height: commit.block_height,
            status: BatchFinalizationStatus::Pending,
            attempts: 0,
            finalize_after_unix,
            message_hash: None,
            message_hash_norm: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("postgres storage failed: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("postgres migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("{field} value {value} does not fit into postgres bigint")]
    BigIntOverflow { field: &'static str, value: u64 },
    #[error("{field} contains invalid hash value {value}")]
    InvalidHash { field: &'static str, value: String },
    #[error("{field} contains invalid status value {value}")]
    InvalidStatus { field: &'static str, value: String },
    #[error("{resource} already exists with different data")]
    Conflict { resource: &'static str },
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn health_check(&self) -> Result<(), StorageError>;
    async fn save_block(&self, block: L2Block) -> Result<(), StorageError>;
    async fn get_block(&self, height: u64) -> Result<Option<L2Block>, StorageError>;
    async fn get_transaction(
        &self,
        hash: Hash32,
    ) -> Result<Option<StoredTransaction>, StorageError>;
    async fn list_account_transactions(
        &self,
        account_id: Hash32,
        before_height: Option<u64>,
        before_index: Option<usize>,
        limit: usize,
    ) -> Result<Vec<StoredTransaction>, StorageError>;
    async fn get_withdrawal_proof(
        &self,
        withdrawal_id: Hash32,
    ) -> Result<Option<WithdrawalProof>, StorageError>;
    async fn save_deposit(&self, deposit: DepositEvent) -> Result<bool, StorageError>;
    async fn save_ent_faucet_grant(
        &self,
        account_id: Hash32,
        amount: u128,
    ) -> Result<bool, StorageError>;
    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError>;
    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError>;
    async fn get_batch_commit(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchCommitRecord>, StorageError>;
    async fn list_batch_commits(
        &self,
        statuses: &[BatchCommitStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchCommitRecord>, StorageError>;
    async fn latest_batch_commit(
        &self,
        statuses: &[BatchCommitStatus],
    ) -> Result<Option<BatchCommitRecord>, StorageError>;
    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError>;
    async fn get_batch_finalization(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchFinalizationRecord>, StorageError>;
    async fn list_batch_finalizations(
        &self,
        statuses: &[BatchFinalizationStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchFinalizationRecord>, StorageError>;
    async fn latest_batch_finalization(
        &self,
        statuses: &[BatchFinalizationStatus],
    ) -> Result<Option<BatchFinalizationRecord>, StorageError>;
    async fn save_batch_finalization(
        &self,
        record: BatchFinalizationRecord,
    ) -> Result<(), StorageError>;
    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError>;
    async fn get_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, StorageError>;
    async fn save_observer_checkpoint(
        &self,
        checkpoint: ObserverCheckpoint,
    ) -> Result<(), StorageError>;
    async fn latest_observer_checkpoint(&self) -> Result<Option<ObserverCheckpoint>, StorageError>;
}

pub type DynStorage = Arc<dyn Storage>;

pub async fn build_storage(config: &NodeConfig) -> Result<DynStorage, StorageError> {
    let storage = PostgresStorage::connect(config.database_url.expose()).await?;
    Ok(Arc::new(storage))
}

#[derive(Debug, Default)]
pub struct InMemoryStorage {
    blocks: RwLock<Vec<L2Block>>,
    batch_commits: RwLock<BTreeMap<u64, BatchCommitRecord>>,
    batch_finalizations: RwLock<BTreeMap<u64, BatchFinalizationRecord>>,
    batch_payloads: RwLock<BTreeMap<u64, StoredBatchPayload>>,
    observer_checkpoints: RwLock<BTreeMap<u64, ObserverCheckpoint>>,
    deposits: RwLock<BTreeMap<Hash32, DepositEvent>>,
    deposit_l1_keys: RwLock<BTreeSet<(Hash32, u64)>>,
    ent_faucet_grants: RwLock<BTreeMap<Hash32, u128>>,
    cursors: RwLock<BTreeMap<String, L1Cursor>>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn health_check(&self) -> Result<(), StorageError> {
        Ok(())
    }

    async fn save_block(&self, block: L2Block) -> Result<(), StorageError> {
        let pending_record = BatchCommitRecord::pending(&block);
        let mut blocks = self.blocks.write().await;
        if let Some(existing) = blocks
            .iter_mut()
            .find(|existing| existing.header.height == block.header.height)
        {
            *existing = block;
        } else {
            blocks.push(block);
            blocks.sort_by_key(|block| block.header.height);
        }
        if let Some(record) = pending_record {
            self.batch_commits
                .write()
                .await
                .entry(record.batch_no)
                .or_insert(record);
        }
        Ok(())
    }

    async fn get_block(&self, height: u64) -> Result<Option<L2Block>, StorageError> {
        let blocks = self.blocks.read().await;
        Ok(blocks
            .iter()
            .find(|block| block.header.height == height)
            .cloned())
    }

    async fn get_transaction(
        &self,
        hash: Hash32,
    ) -> Result<Option<StoredTransaction>, StorageError> {
        let blocks = self.blocks.read().await;
        for block in blocks.iter() {
            if let Some((index, transaction)) = block
                .transactions
                .iter()
                .enumerate()
                .find(|(_, transaction)| transaction.tx_hash() == hash)
            {
                return Ok(Some(StoredTransaction {
                    block_height: block.header.height,
                    block_timestamp: block.header.timestamp,
                    block_hash: block.header.block_hash(),
                    tx_index: index,
                    transaction: transaction.clone(),
                    receipt: block.receipts.get(index).cloned(),
                }));
            }
        }
        Ok(None)
    }

    async fn list_account_transactions(
        &self,
        account_id: Hash32,
        before_height: Option<u64>,
        before_index: Option<usize>,
        limit: usize,
    ) -> Result<Vec<StoredTransaction>, StorageError> {
        let blocks = self.blocks.read().await;
        let mut out = Vec::new();
        for block in blocks.iter().rev() {
            for (index, transaction) in block.transactions.iter().enumerate().rev() {
                if !is_before_transaction_cursor(
                    block.header.height,
                    index,
                    before_height,
                    before_index,
                ) || !transaction_touches_account(transaction, account_id)
                {
                    continue;
                }
                out.push(StoredTransaction {
                    block_height: block.header.height,
                    block_timestamp: block.header.timestamp,
                    block_hash: block.header.block_hash(),
                    tx_index: index,
                    transaction: transaction.clone(),
                    receipt: block.receipts.get(index).cloned(),
                });
                if out.len() >= limit {
                    return Ok(out);
                }
            }
        }
        Ok(out)
    }

    async fn get_withdrawal_proof(
        &self,
        withdrawal_id: Hash32,
    ) -> Result<Option<WithdrawalProof>, StorageError> {
        let blocks = self.blocks.read().await;
        for block in blocks.iter() {
            if let Some(proof) = block.withdrawal_proof(withdrawal_id) {
                return Ok(Some(proof));
            }
        }
        Ok(None)
    }

    async fn save_deposit(&self, deposit: DepositEvent) -> Result<bool, StorageError> {
        let mut deposits = self.deposits.write().await;
        let mut l1_keys = self.deposit_l1_keys.write().await;
        let l1_key = (deposit.l1_tx_hash, deposit.l1_lt);
        if deposits.contains_key(&deposit.deposit_id) || l1_keys.contains(&l1_key) {
            return Ok(false);
        }
        l1_keys.insert(l1_key);
        deposits.insert(deposit.deposit_id, deposit);
        Ok(true)
    }

    async fn save_ent_faucet_grant(
        &self,
        account_id: Hash32,
        amount: u128,
    ) -> Result<bool, StorageError> {
        let mut grants = self.ent_faucet_grants.write().await;
        Ok(grants.insert(account_id, amount).is_none())
    }

    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError> {
        Ok(self.cursors.read().await.get(source).cloned())
    }

    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError> {
        self.cursors.write().await.insert(source.to_owned(), cursor);
        Ok(())
    }

    async fn get_batch_commit(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchCommitRecord>, StorageError> {
        Ok(self.batch_commits.read().await.get(&batch_no).cloned())
    }

    async fn list_batch_commits(
        &self,
        statuses: &[BatchCommitStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchCommitRecord>, StorageError> {
        let limit = limit as usize;
        Ok(self
            .batch_commits
            .read()
            .await
            .values()
            .filter(|record| statuses.contains(&record.status))
            .filter(|record| record.attempts < max_attempts)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn latest_batch_commit(
        &self,
        statuses: &[BatchCommitStatus],
    ) -> Result<Option<BatchCommitRecord>, StorageError> {
        Ok(self
            .batch_commits
            .read()
            .await
            .values()
            .filter(|record| statuses.is_empty() || statuses.contains(&record.status))
            .max_by_key(|record| record.batch_no)
            .cloned())
    }

    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError> {
        self.batch_commits
            .write()
            .await
            .insert(record.batch_no, record);
        Ok(())
    }

    async fn get_batch_finalization(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchFinalizationRecord>, StorageError> {
        Ok(self
            .batch_finalizations
            .read()
            .await
            .get(&batch_no)
            .cloned())
    }

    async fn list_batch_finalizations(
        &self,
        statuses: &[BatchFinalizationStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchFinalizationRecord>, StorageError> {
        let limit = limit as usize;
        Ok(self
            .batch_finalizations
            .read()
            .await
            .values()
            .filter(|record| statuses.contains(&record.status))
            .filter(|record| record.attempts < max_attempts)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn latest_batch_finalization(
        &self,
        statuses: &[BatchFinalizationStatus],
    ) -> Result<Option<BatchFinalizationRecord>, StorageError> {
        Ok(self
            .batch_finalizations
            .read()
            .await
            .values()
            .filter(|record| statuses.is_empty() || statuses.contains(&record.status))
            .max_by_key(|record| record.batch_no)
            .cloned())
    }

    async fn save_batch_finalization(
        &self,
        record: BatchFinalizationRecord,
    ) -> Result<(), StorageError> {
        self.batch_finalizations
            .write()
            .await
            .insert(record.batch_no, record);
        Ok(())
    }

    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError> {
        let mut payloads = self.batch_payloads.write().await;
        if let Some(existing) = payloads.get_mut(&payload.block_height) {
            if existing.has_same_canonical_payload(&payload)
                && !existing.has_public_ref_conflict(&payload)
            {
                existing.merge_public_metadata_from(&payload);
                return Ok(false);
            }
            return Err(StorageError::Conflict {
                resource: "batch payload",
            });
        }
        payloads.insert(payload.block_height, payload);
        Ok(true)
    }

    async fn get_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, StorageError> {
        Ok(self.batch_payloads.read().await.get(&block_height).cloned())
    }

    async fn save_observer_checkpoint(
        &self,
        checkpoint: ObserverCheckpoint,
    ) -> Result<(), StorageError> {
        self.observer_checkpoints
            .write()
            .await
            .insert(checkpoint.next_batch_no, checkpoint);
        Ok(())
    }

    async fn latest_observer_checkpoint(&self) -> Result<Option<ObserverCheckpoint>, StorageError> {
        Ok(self
            .observer_checkpoints
            .read()
            .await
            .values()
            .max_by_key(|checkpoint| checkpoint.next_batch_no)
            .cloned())
    }
}

fn is_before_transaction_cursor(
    block_height: u64,
    tx_index: usize,
    before_height: Option<u64>,
    before_index: Option<usize>,
) -> bool {
    let Some(before_height) = before_height else {
        return true;
    };
    block_height < before_height
        || (block_height == before_height && tx_index < before_index.unwrap_or(usize::MAX))
}

fn transaction_touches_account(transaction: &SignedL2Transaction, account_id: Hash32) -> bool {
    if transaction.from == Some(account_id) {
        return true;
    }
    match &transaction.kind {
        l2_core::L2TransactionKind::Deposit { recipient, .. } => *recipient == account_id,
        l2_core::L2TransactionKind::Transfer { to, .. } => *to == account_id,
        l2_core::L2TransactionKind::Withdraw { .. } => false,
        l2_core::L2TransactionKind::DeployContract { contract, .. } => *contract == account_id,
        l2_core::L2TransactionKind::CallContract { contract, .. } => *contract == account_id,
        l2_core::L2TransactionKind::RotatePublicKey { .. } => false,
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
