use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, Receipt, SignedL2Transaction, WithdrawalProof};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::config::NodeConfig;

mod memory;
mod postgres;
mod postgres_batches;
mod postgres_blocks;
mod postgres_deposits;
mod postgres_utils;

pub use memory::InMemoryStorage;
pub use postgres::PostgresStorage;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub block_height: u64,
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
pub struct BatchCommitRecord {
    pub batch_no: u64,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub status: BatchCommitStatus,
    pub attempts: u32,
    pub message_hash: Option<Hash32>,
    pub message_hash_norm: Option<Hash32>,
    pub last_error: Option<String>,
    pub l1_committed_at: Option<u64>,
    pub finalization_eligible_at: Option<u64>,
    pub finalization_status: BatchFinalizationStatus,
    pub finalization_attempts: u32,
    pub finalize_message_hash: Option<Hash32>,
    pub finalize_message_hash_norm: Option<Hash32>,
    pub finalization_last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredBatchPayload {
    pub block_height: u64,
    pub block_hash: Hash32,
    pub data_hash: Hash32,
    pub payload_bytes: Vec<u8>,
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
            l1_committed_at: None,
            finalization_eligible_at: None,
            finalization_status: BatchFinalizationStatus::Pending,
            finalization_attempts: 0,
            finalize_message_hash: None,
            finalize_message_hash_norm: None,
            finalization_last_error: None,
        })
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
    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError>;
    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError>;
    async fn get_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, StorageError>;
}

pub type DynStorage = Arc<dyn Storage>;

pub async fn build_storage(config: &NodeConfig) -> Result<DynStorage, StorageError> {
    let storage = PostgresStorage::connect(config.database_url.expose()).await?;
    Ok(Arc::new(storage))
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
