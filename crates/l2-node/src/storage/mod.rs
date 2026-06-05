use async_trait::async_trait;
use l2_core::{Account, DepositEvent, Hash32, L2Block, WithdrawalProof};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::NodeConfig;
use query::{is_before_transaction_cursor, transaction_touches_account};

mod contracts;
mod da_payload;
mod faucet;
mod observer;
mod postgres;
#[path = "postgres/contracts.rs"]
mod postgres_contracts;
#[path = "postgres/da.rs"]
mod postgres_da;
#[path = "postgres/faucet.rs"]
mod postgres_faucet;
#[path = "postgres/finalization.rs"]
mod postgres_finalization;
#[path = "postgres/internal_queue.rs"]
mod postgres_internal_queue;
#[path = "postgres/observer.rs"]
mod postgres_observer;
#[path = "postgres/util.rs"]
mod postgres_util;
mod query;
mod types;

pub use contracts::{StoredContractCodeCell, StoredContractDataCell, StoredContractState};
pub use faucet::{
    EntFaucetClaimRecord, EntFaucetClaimSaveResult, EntFaucetClaimSaveStatus, EntFaucetClaimStatus,
};
pub use observer::ObserverCheckpoint;
pub use postgres::PostgresStorage;
pub use types::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus,
    DynStorage, ExplorerStorageStats, InternalQueueSnapshotRecord, L1Cursor, Storage, StorageError,
    StoredBatchPayload, StoredTransaction, VerifierSourceFile, VerifierStatus,
    VerifierSubmissionRecord,
};

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
    contract_code_cells: RwLock<BTreeMap<Hash32, StoredContractCodeCell>>,
    contract_data_cells: RwLock<BTreeMap<Hash32, StoredContractDataCell>>,
    contract_accounts: RwLock<BTreeMap<Hash32, (Account, u64)>>,
    verifier_submissions: RwLock<BTreeMap<Hash32, VerifierSubmissionRecord>>,
    internal_queue_snapshots: RwLock<BTreeMap<u64, InternalQueueSnapshotRecord>>,
    observer_checkpoints: RwLock<BTreeMap<u64, ObserverCheckpoint>>,
    deposits: RwLock<BTreeMap<Hash32, DepositEvent>>,
    deposit_l1_keys: RwLock<BTreeSet<(Hash32, u64)>>,
    ent_faucet_grants: RwLock<BTreeMap<Hash32, u128>>,
    ent_faucet_claims: RwLock<BTreeMap<String, EntFaucetClaimRecord>>,
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

    async fn explorer_storage_stats(&self) -> Result<ExplorerStorageStats, StorageError> {
        let blocks = self.blocks.read().await;
        Ok(ExplorerStorageStats {
            block_count: blocks.len() as u64,
            transaction_count: blocks
                .iter()
                .map(|block| block.transactions.len() as u64)
                .sum(),
            deposit_count: blocks
                .iter()
                .flat_map(|block| &block.transactions)
                .filter(|tx| matches!(tx.kind, l2_core::L2TransactionKind::Deposit { .. }))
                .count() as u64,
            withdrawal_count: blocks
                .iter()
                .map(|block| block.withdrawals.len() as u64)
                .sum(),
        })
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

    async fn save_ent_faucet_batch_claim(
        &self,
        mut record: EntFaucetClaimRecord,
        deposit: DepositEvent,
    ) -> Result<EntFaucetClaimSaveResult, StorageError> {
        let mut claims = self.ent_faucet_claims.write().await;
        if let Some(existing) = claims.get(&record.claim_id) {
            return Ok(EntFaucetClaimSaveResult {
                status: EntFaucetClaimSaveStatus::DuplicateClaim,
                record: existing.clone(),
            });
        }

        let mut grants = self.ent_faucet_grants.write().await;
        if grants.contains_key(&record.account_id) {
            record.status = EntFaucetClaimStatus::DuplicateAccount;
            claims.insert(record.claim_id.clone(), record.clone());
            return Ok(EntFaucetClaimSaveResult {
                status: EntFaucetClaimSaveStatus::DuplicateAccount,
                record,
            });
        }

        let mut deposits = self.deposits.write().await;
        let mut l1_keys = self.deposit_l1_keys.write().await;
        let l1_key = (deposit.l1_tx_hash, deposit.l1_lt);
        if deposits.contains_key(&deposit.deposit_id) || l1_keys.contains(&l1_key) {
            return Err(StorageError::Conflict {
                resource: "ent faucet deposit",
            });
        }

        record.status = EntFaucetClaimStatus::Granted;
        grants.insert(record.account_id, record.amount_base_units);
        l1_keys.insert(l1_key);
        deposits.insert(deposit.deposit_id, deposit);
        claims.insert(record.claim_id.clone(), record.clone());
        Ok(EntFaucetClaimSaveResult {
            status: EntFaucetClaimSaveStatus::Granted,
            record,
        })
    }

    async fn list_ent_faucet_claims(
        &self,
        limit: u32,
    ) -> Result<Vec<EntFaucetClaimRecord>, StorageError> {
        let limit = limit as usize;
        Ok(self
            .ent_faucet_claims
            .read()
            .await
            .values()
            .rev()
            .take(limit)
            .cloned()
            .collect())
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

    async fn save_contract_state(&self, record: StoredContractState) -> Result<(), StorageError> {
        {
            let mut code_cells = self.contract_code_cells.write().await;
            if let Some(existing) = code_cells.get(&record.code_cell.code_hash) {
                if existing.code_boc_base64 != record.code_cell.code_boc_base64
                    || existing.size_bytes != record.code_cell.size_bytes
                {
                    return Err(StorageError::Conflict {
                        resource: "contract code cell",
                    });
                }
            } else {
                code_cells.insert(record.code_cell.code_hash, record.code_cell.clone());
            }
        }
        {
            let mut data_cells = self.contract_data_cells.write().await;
            if let Some(existing) = data_cells.get(&record.data_cell.data_hash) {
                if existing.data_boc_base64 != record.data_cell.data_boc_base64
                    || existing.storage_root != record.data_cell.storage_root
                    || existing.size_bytes != record.data_cell.size_bytes
                {
                    return Err(StorageError::Conflict {
                        resource: "contract data cell",
                    });
                }
            } else {
                data_cells.insert(record.data_cell.data_hash, record.data_cell.clone());
            }
        }
        let mut accounts = self.contract_accounts.write().await;
        if accounts
            .get(&record.account_id)
            .is_none_or(|(_, height)| *height <= record.last_block_height)
        {
            accounts.insert(
                record.account_id,
                (record.account.clone(), record.last_block_height),
            );
        }
        Ok(())
    }

    async fn get_contract_state(
        &self,
        account_id: Hash32,
    ) -> Result<Option<StoredContractState>, StorageError> {
        let Some((account, last_block_height)) = self
            .contract_accounts
            .read()
            .await
            .get(&account_id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(code_cell) = self
            .contract_code_cells
            .read()
            .await
            .get(&account.code_hash)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(data_cell) = self
            .contract_data_cells
            .read()
            .await
            .get(&account.data_hash)
            .cloned()
        else {
            return Ok(None);
        };
        Ok(Some(StoredContractState {
            account_id,
            account,
            code_cell,
            data_cell,
            last_block_height,
        }))
    }

    async fn save_verifier_submission(
        &self,
        submission: VerifierSubmissionRecord,
    ) -> Result<(), StorageError> {
        self.verifier_submissions
            .write()
            .await
            .entry(submission.submission_id)
            .or_insert(submission);
        Ok(())
    }

    async fn get_verifier_source(
        &self,
        code_hash: Hash32,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError> {
        Ok(self
            .verifier_submissions
            .read()
            .await
            .values()
            .filter(|submission| submission.code_hash == code_hash)
            .filter(|submission| submission.status != VerifierStatus::Rejected)
            .max_by_key(|submission| match submission.status {
                VerifierStatus::Verified => 2u8,
                VerifierStatus::Pending => 1u8,
                VerifierStatus::Rejected => 0u8,
            })
            .cloned())
    }

    async fn review_verifier_submission(
        &self,
        submission_id: Hash32,
        status: VerifierStatus,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError> {
        let mut submissions = self.verifier_submissions.write().await;
        let Some(submission) = submissions.get_mut(&submission_id) else {
            return Ok(None);
        };
        submission.status = status;
        Ok(Some(submission.clone()))
    }

    async fn save_internal_queue_snapshot(
        &self,
        record: InternalQueueSnapshotRecord,
    ) -> Result<(), StorageError> {
        self.internal_queue_snapshots
            .write()
            .await
            .insert(record.block_height, record);
        Ok(())
    }

    async fn latest_internal_queue_snapshot(
        &self,
    ) -> Result<Option<InternalQueueSnapshotRecord>, StorageError> {
        Ok(self
            .internal_queue_snapshots
            .read()
            .await
            .values()
            .max_by_key(|record| record.block_height)
            .cloned())
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
