use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, Receipt, SignedL2Transaction, WithdrawalProof};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::config::NodeConfig;

mod postgres;

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
}

#[async_trait]
pub trait Storage: Send + Sync {
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
    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError>;
    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError>;
}

pub type DynStorage = Arc<dyn Storage>;

pub async fn build_storage(config: &NodeConfig) -> Result<DynStorage, StorageError> {
    let storage = PostgresStorage::connect(config.database_url.expose()).await?;
    Ok(Arc::new(storage))
}

#[derive(Debug, Default)]
pub struct InMemoryStorage {
    blocks: RwLock<Vec<L2Block>>,
    deposits: RwLock<BTreeMap<Hash32, DepositEvent>>,
    cursors: RwLock<BTreeMap<String, L1Cursor>>,
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn save_block(&self, block: L2Block) -> Result<(), StorageError> {
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
                    transaction: transaction.clone(),
                    receipt: block.receipts.get(index).cloned(),
                }));
            }
        }
        Ok(None)
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
        Ok(deposits.insert(deposit.deposit_id, deposit).is_none())
    }

    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError> {
        Ok(self.cursors.read().await.get(source).cloned())
    }

    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError> {
        self.cursors.write().await.insert(source.to_owned(), cursor);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_core::{crypto::sha256_bytes, L2Block};

    fn deposit_event() -> DepositEvent {
        DepositEvent {
            deposit_id: sha256_bytes(b"deposit"),
            asset_id: 0,
            recipient: sha256_bytes(b"recipient"),
            amount: 100,
            l1_tx_hash: sha256_bytes(b"l1-tx"),
            l1_lt: 1,
        }
    }

    #[tokio::test]
    async fn memory_storage_deposit_idempotency_rejects_replay() {
        let storage = InMemoryStorage::default();
        let deposit = deposit_event();

        assert!(storage.save_deposit(deposit.clone()).await.unwrap());
        assert!(!storage.save_deposit(deposit).await.unwrap());
    }

    #[tokio::test]
    async fn memory_storage_block_lookup_is_reproducible() {
        let storage = InMemoryStorage::default();
        let block = L2Block::new(
            7,
            Hash32::ZERO,
            Hash32::ZERO,
            sha256_bytes(b"state"),
            vec![],
            vec![],
            vec![],
            sha256_bytes(b"data"),
            100,
        );
        storage.save_block(block.clone()).await.unwrap();

        let loaded = storage.get_block(7).await.unwrap().expect("block");
        assert_eq!(loaded.header.block_hash(), block.header.block_hash());
        assert!(storage.get_block(8).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn memory_storage_cursor_roundtrip() {
        let storage = InMemoryStorage::default();
        let cursor = L1Cursor {
            lt: 42,
            hash: sha256_bytes(b"cursor"),
        };

        storage
            .set_l1_cursor("vault", cursor.clone())
            .await
            .unwrap();
        assert_eq!(storage.get_l1_cursor("vault").await.unwrap(), Some(cursor));
        assert!(storage.get_l1_cursor("missing").await.unwrap().is_none());
    }
}
