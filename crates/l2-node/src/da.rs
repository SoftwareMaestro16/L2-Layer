use crate::config::NodeConfig;
use crate::storage::{DynStorage, StorageError, StoredBatchPayload};
use async_trait::async_trait;
use l2_core::{canonical_batch_data_bytes, canonical_batch_data_hash, Hash32, L2Block};
use std::sync::Arc;
use thiserror::Error;

pub const DEFAULT_DA_MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataAvailabilityConfig {
    pub max_payload_bytes: usize,
}

impl DataAvailabilityConfig {
    pub fn from_node_config(config: &NodeConfig) -> Self {
        Self {
            max_payload_bytes: config.da_max_payload_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchDaRef {
    pub block_height: u64,
    pub block_hash: Hash32,
    pub data_hash: Hash32,
    pub payload_size: usize,
}

#[async_trait]
pub trait DaWriter: Send + Sync {
    async fn write_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError>;
}

#[async_trait]
pub trait DaReader: Send + Sync {
    async fn read_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, DaError>;
}

#[async_trait]
pub trait DaVerifier: Send + Sync {
    async fn verify_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError>;
}

pub trait DataAvailability: DaWriter + DaReader + DaVerifier {}

impl<T> DataAvailability for T where T: DaWriter + DaReader + DaVerifier {}

pub type DynDa = Arc<dyn DataAvailability>;

#[derive(Clone)]
pub struct StorageDaStore {
    storage: DynStorage,
    config: DataAvailabilityConfig,
}

impl StorageDaStore {
    pub fn new(storage: DynStorage, config: DataAvailabilityConfig) -> Self {
        Self { storage, config }
    }

    fn payload_for_block(&self, block: &L2Block) -> Result<Vec<u8>, DaError> {
        let payload = canonical_batch_data_bytes(&block.transactions, &block.receipts);
        if payload.len() > self.config.max_payload_bytes {
            return Err(DaError::PayloadTooLarge {
                bytes: payload.len(),
                max: self.config.max_payload_bytes,
            });
        }
        let actual = canonical_batch_data_hash(&block.transactions, &block.receipts);
        if actual != block.header.data_hash {
            return Err(DaError::HashMismatch {
                expected: block.header.data_hash,
                actual,
            });
        }
        Ok(payload)
    }

    fn record_for_block(&self, block: &L2Block) -> Result<StoredBatchPayload, DaError> {
        Ok(StoredBatchPayload {
            block_height: block.header.height,
            block_hash: block.header.block_hash(),
            data_hash: block.header.data_hash,
            payload_bytes: self.payload_for_block(block)?,
        })
    }
}

#[async_trait]
impl DaWriter for StorageDaStore {
    async fn write_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError> {
        let record = self.record_for_block(block)?;
        let payload_size = record.payload_bytes.len();
        self.storage.save_batch_payload(record.clone()).await?;
        Ok(BatchDaRef {
            block_height: record.block_height,
            block_hash: record.block_hash,
            data_hash: record.data_hash,
            payload_size,
        })
    }
}

#[async_trait]
impl DaReader for StorageDaStore {
    async fn read_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, DaError> {
        Ok(self.storage.get_batch_payload(block_height).await?)
    }
}

#[async_trait]
impl DaVerifier for StorageDaStore {
    async fn verify_batch_payload(&self, block: &L2Block) -> Result<BatchDaRef, DaError> {
        let expected = self.record_for_block(block)?;
        let Some(stored) = self.read_batch_payload(block.header.height).await? else {
            return Err(DaError::Unavailable);
        };
        if stored.block_hash != expected.block_hash {
            return Err(DaError::BlockHashMismatch {
                expected: expected.block_hash,
                actual: stored.block_hash,
            });
        }
        if stored.data_hash != expected.data_hash {
            return Err(DaError::HashMismatch {
                expected: expected.data_hash,
                actual: stored.data_hash,
            });
        }
        if stored.payload_bytes != expected.payload_bytes {
            let actual = l2_core::crypto::hash_domain("l2.batch.data.v1", &[&stored.payload_bytes]);
            return Err(DaError::HashMismatch {
                expected: expected.data_hash,
                actual,
            });
        }
        Ok(BatchDaRef {
            block_height: expected.block_height,
            block_hash: expected.block_hash,
            data_hash: expected.data_hash,
            payload_size: expected.payload_bytes.len(),
        })
    }
}

#[derive(Debug, Error)]
pub enum DaError {
    #[error("batch payload is unavailable")]
    Unavailable,
    #[error("batch payload is {bytes} bytes, max is {max} bytes")]
    PayloadTooLarge { bytes: usize, max: usize },
    #[error("batch payload hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: Hash32, actual: Hash32 },
    #[error("batch block hash mismatch: expected {expected}, got {actual}")]
    BlockHashMismatch { expected: Hash32, actual: Hash32 },
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
}

#[cfg(test)]
#[path = "da_tests.rs"]
mod tests;
