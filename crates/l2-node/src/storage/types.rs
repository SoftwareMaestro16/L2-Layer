use async_trait::async_trait;
use l2_core::{
    DepositEvent, Hash32, InternalMessageQueueSnapshot, L2Block, Receipt, SignedL2Transaction,
    WithdrawalProof,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use super::{ObserverCheckpoint, StoredContractState};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub block_height: u64,
    pub block_timestamp: u64,
    pub block_hash: Hash32,
    pub tx_index: usize,
    pub transaction: SignedL2Transaction,
    pub receipt: Option<Receipt>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExplorerStorageStats {
    pub block_count: u64,
    pub transaction_count: u64,
    pub deposit_count: u64,
    pub withdrawal_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierStatus {
    Pending,
    Verified,
    Rejected,
}

impl VerifierStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "verified" => Some(Self::Verified),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifierSourceFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifierSubmissionRecord {
    pub submission_id: Hash32,
    pub code_hash: Hash32,
    pub account_id: Option<Hash32>,
    pub status: VerifierStatus,
    pub files: Vec<VerifierSourceFile>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InternalQueueSnapshotRecord {
    pub block_height: u64,
    pub queue: InternalMessageQueueSnapshot,
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
    #[error("{field} is missing for contract state persistence")]
    MissingContractCell { field: &'static str },
    #[error("{field} is invalid for contract state persistence: {reason}")]
    InvalidContractCell { field: &'static str, reason: String },
    #[error("{field} hash mismatch: expected {expected}, actual {actual}")]
    ContractCellHashMismatch {
        field: &'static str,
        expected: Hash32,
        actual: Hash32,
    },
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
    async fn explorer_storage_stats(&self) -> Result<ExplorerStorageStats, StorageError>;
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
    async fn save_contract_state(&self, record: StoredContractState) -> Result<(), StorageError>;
    async fn get_contract_state(
        &self,
        account_id: Hash32,
    ) -> Result<Option<StoredContractState>, StorageError>;
    async fn save_verifier_submission(
        &self,
        submission: VerifierSubmissionRecord,
    ) -> Result<(), StorageError>;
    async fn get_verifier_source(
        &self,
        code_hash: Hash32,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError>;
    async fn review_verifier_submission(
        &self,
        submission_id: Hash32,
        status: VerifierStatus,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError>;
    async fn save_internal_queue_snapshot(
        &self,
        record: InternalQueueSnapshotRecord,
    ) -> Result<(), StorageError>;
    async fn latest_internal_queue_snapshot(
        &self,
    ) -> Result<Option<InternalQueueSnapshotRecord>, StorageError>;
    async fn save_observer_checkpoint(
        &self,
        checkpoint: ObserverCheckpoint,
    ) -> Result<(), StorageError>;
    async fn latest_observer_checkpoint(&self) -> Result<Option<ObserverCheckpoint>, StorageError>;
}

pub type DynStorage = Arc<dyn Storage>;
