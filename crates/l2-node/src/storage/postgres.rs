use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, WithdrawalProof};
use sqlx::postgres::{PgPool, PgPoolOptions};

use super::{
    BatchCommitRecord, BatchCommitStatus, L1Cursor, Storage, StorageError, StoredBatchPayload,
    StoredTransaction,
};

#[derive(Clone, Debug)]
pub struct PostgresStorage {
    pub(super) pool: PgPool,
}

impl PostgresStorage {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    #[allow(dead_code)]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn health_check(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn save_block(&self, block: L2Block) -> Result<(), StorageError> {
        super::postgres_blocks::save_block(self, block).await
    }

    async fn get_block(&self, height: u64) -> Result<Option<L2Block>, StorageError> {
        super::postgres_blocks::get_block(self, height).await
    }

    async fn get_transaction(
        &self,
        hash: Hash32,
    ) -> Result<Option<StoredTransaction>, StorageError> {
        super::postgres_blocks::get_transaction(self, hash).await
    }

    async fn get_withdrawal_proof(
        &self,
        withdrawal_id: Hash32,
    ) -> Result<Option<WithdrawalProof>, StorageError> {
        super::postgres_blocks::get_withdrawal_proof(self, withdrawal_id).await
    }

    async fn save_deposit(&self, deposit: DepositEvent) -> Result<bool, StorageError> {
        super::postgres_deposits::save_deposit(self, deposit).await
    }

    async fn save_ent_faucet_grant(
        &self,
        account_id: Hash32,
        amount: u128,
    ) -> Result<bool, StorageError> {
        super::postgres_deposits::save_ent_faucet_grant(self, account_id, amount).await
    }

    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError> {
        super::postgres_deposits::get_l1_cursor(self, source).await
    }

    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError> {
        super::postgres_deposits::set_l1_cursor(self, source, cursor).await
    }

    async fn get_batch_commit(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchCommitRecord>, StorageError> {
        super::postgres_batches::get_batch_commit(self, batch_no).await
    }

    async fn list_batch_commits(
        &self,
        statuses: &[BatchCommitStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchCommitRecord>, StorageError> {
        super::postgres_batches::list_batch_commits(self, statuses, max_attempts, limit).await
    }

    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError> {
        super::postgres_batches::save_batch_commit(self, record).await
    }

    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError> {
        super::postgres_batches::save_batch_payload(self, payload).await
    }

    async fn get_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, StorageError> {
        super::postgres_batches::get_batch_payload(self, block_height).await
    }
}
