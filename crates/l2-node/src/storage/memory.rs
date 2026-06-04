use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, WithdrawalProof};
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::RwLock;

use super::{
    BatchCommitRecord, BatchCommitStatus, L1Cursor, Storage, StorageError, StoredBatchPayload,
    StoredTransaction,
};

#[derive(Debug, Default)]
pub struct InMemoryStorage {
    blocks: RwLock<Vec<L2Block>>,
    batch_commits: RwLock<BTreeMap<u64, BatchCommitRecord>>,
    batch_payloads: RwLock<BTreeMap<u64, StoredBatchPayload>>,
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
        Ok(self
            .batch_commits
            .read()
            .await
            .values()
            .filter(|record| statuses.contains(&record.status))
            .filter(|record| record.attempts < max_attempts)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError> {
        self.batch_commits
            .write()
            .await
            .insert(record.batch_no, record);
        Ok(())
    }

    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError> {
        let mut payloads = self.batch_payloads.write().await;
        if let Some(existing) = payloads.get(&payload.block_height) {
            if existing == &payload {
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
}
