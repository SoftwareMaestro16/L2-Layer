use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, WithdrawalProof};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

use super::{
    BatchCommitRecord, BatchCommitStatus, L1Cursor, Storage, StorageError, StoredTransaction,
};

#[derive(Clone, Debug)]
pub struct PostgresStorage {
    pool: PgPool,
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
    async fn save_block(&self, block: L2Block) -> Result<(), StorageError> {
        let block_height = checked_i64(block.header.height, "block_height")?;
        let block_hash = block.header.block_hash().to_hex();
        let block_json = serde_json::to_value(&block)?;
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO l2_blocks (
                height, block_hash, prev_block_hash, state_root, data_hash, block_json
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (height) DO UPDATE SET
                block_hash = EXCLUDED.block_hash,
                prev_block_hash = EXCLUDED.prev_block_hash,
                state_root = EXCLUDED.state_root,
                data_hash = EXCLUDED.data_hash,
                block_json = EXCLUDED.block_json
            "#,
        )
        .bind(block_height)
        .bind(block_hash)
        .bind(block.header.prev_block_hash.to_hex())
        .bind(block.header.state_root.to_hex())
        .bind(block.header.data_hash.to_hex())
        .bind(block_json)
        .execute(&mut *tx)
        .await?;

        for (index, transaction) in block.transactions.iter().enumerate() {
            let tx_index = checked_i32(index, "tx_index")?;
            let receipt = block.receipts.get(index).cloned();
            sqlx::query(
                r#"
                INSERT INTO l2_transactions (
                    tx_hash, block_height, tx_index, tx_json, receipt_json
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (tx_hash) DO UPDATE SET
                    block_height = EXCLUDED.block_height,
                    tx_index = EXCLUDED.tx_index,
                    tx_json = EXCLUDED.tx_json,
                    receipt_json = EXCLUDED.receipt_json
                "#,
            )
            .bind(transaction.tx_hash().to_hex())
            .bind(block_height)
            .bind(tx_index)
            .bind(serde_json::to_value(transaction)?)
            .bind(serde_json::to_value(receipt)?)
            .execute(&mut *tx)
            .await?;
        }

        for (index, withdrawal) in block.withdrawals.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO l2_withdrawals (
                    withdrawal_id, block_height, withdrawal_index, withdrawal_json
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (withdrawal_id) DO UPDATE SET
                    block_height = EXCLUDED.block_height,
                    withdrawal_index = EXCLUDED.withdrawal_index,
                    withdrawal_json = EXCLUDED.withdrawal_json
                "#,
            )
            .bind(withdrawal.withdrawal_id.to_hex())
            .bind(block_height)
            .bind(checked_i32(index, "withdrawal_index")?)
            .bind(serde_json::to_value(withdrawal)?)
            .execute(&mut *tx)
            .await?;
        }

        if let Some(record) = BatchCommitRecord::pending(&block) {
            sqlx::query(
                r#"
                INSERT INTO l1_batch_commits (
                    batch_no, block_height, block_hash, status
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (batch_no) DO NOTHING
                "#,
            )
            .bind(checked_i64(record.batch_no, "batch_no")?)
            .bind(checked_i64(record.block_height, "block_height")?)
            .bind(record.block_hash.to_hex())
            .bind(record.status.as_str())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_block(&self, height: u64) -> Result<Option<L2Block>, StorageError> {
        let height = checked_i64(height, "block_height")?;
        let Some(value) = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT block_json FROM l2_blocks WHERE height = $1",
        )
        .bind(height)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(serde_json::from_value(value)?))
    }

    async fn get_transaction(
        &self,
        hash: Hash32,
    ) -> Result<Option<StoredTransaction>, StorageError> {
        let Some(row) = sqlx::query(
            r#"
            SELECT block_height, tx_json, receipt_json
            FROM l2_transactions
            WHERE tx_hash = $1
            "#,
        )
        .bind(hash.to_hex())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        let block_height: i64 = row.try_get("block_height")?;
        let transaction: serde_json::Value = row.try_get("tx_json")?;
        let receipt: Option<serde_json::Value> = row.try_get("receipt_json")?;
        Ok(Some(StoredTransaction {
            block_height: block_height as u64,
            transaction: serde_json::from_value(transaction)?,
            receipt: receipt.map(serde_json::from_value).transpose()?,
        }))
    }

    async fn get_withdrawal_proof(
        &self,
        withdrawal_id: Hash32,
    ) -> Result<Option<WithdrawalProof>, StorageError> {
        let Some(block_height) = sqlx::query_scalar::<_, i64>(
            "SELECT block_height FROM l2_withdrawals WHERE withdrawal_id = $1",
        )
        .bind(withdrawal_id.to_hex())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let Some(block) = self.get_block(block_height as u64).await? else {
            return Ok(None);
        };
        Ok(block.withdrawal_proof(withdrawal_id))
    }

    async fn save_deposit(&self, deposit: DepositEvent) -> Result<bool, StorageError> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            INSERT INTO l2_deposits (
                deposit_id, asset_id, recipient, amount, l1_tx_hash, l1_lt, deposit_json
            )
            VALUES ($1, $2, $3, $4::numeric, $5, $6, $7)
            ON CONFLICT DO NOTHING
            RETURNING deposit_id
            "#,
        )
        .bind(deposit.deposit_id.to_hex())
        .bind(i64::from(deposit.asset_id))
        .bind(deposit.recipient.to_hex())
        .bind(deposit.amount.to_string())
        .bind(deposit.l1_tx_hash.to_hex())
        .bind(checked_i64(deposit.l1_lt, "l1_lt")?)
        .bind(serde_json::to_value(deposit)?)
        .fetch_optional(&self.pool)
        .await?;

        Ok(inserted.is_some())
    }

    async fn save_ent_faucet_grant(
        &self,
        account_id: Hash32,
        amount: u128,
    ) -> Result<bool, StorageError> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            INSERT INTO ent_faucet_grants (account_id, asset_id, amount)
            VALUES ($1, 0, $2::numeric)
            ON CONFLICT (account_id) DO NOTHING
            RETURNING account_id
            "#,
        )
        .bind(account_id.to_hex())
        .bind(amount.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(inserted.is_some())
    }

    async fn get_l1_cursor(&self, source: &str) -> Result<Option<L1Cursor>, StorageError> {
        let Some(row) = sqlx::query("SELECT lt, hash FROM l1_cursors WHERE source = $1")
            .bind(source)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let lt: i64 = row.try_get("lt")?;
        let hash: String = row.try_get("hash")?;
        Ok(Some(L1Cursor {
            lt: lt as u64,
            hash: Hash32::from_hex(&hash).map_err(|_| StorageError::InvalidHash {
                field: "l1_cursors.hash",
                value: hash,
            })?,
        }))
    }

    async fn set_l1_cursor(&self, source: &str, cursor: L1Cursor) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO l1_cursors (source, lt, hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (source) DO UPDATE SET
                lt = EXCLUDED.lt,
                hash = EXCLUDED.hash,
                updated_at = now()
            "#,
        )
        .bind(source)
        .bind(checked_i64(cursor.lt, "cursor_lt")?)
        .bind(cursor.hash.to_hex())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_batch_commit(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchCommitRecord>, StorageError> {
        let Some(row) = sqlx::query(
            r#"
            SELECT batch_no, block_height, block_hash, status, attempts,
                   message_hash, message_hash_norm, last_error
            FROM l1_batch_commits
            WHERE batch_no = $1
            "#,
        )
        .bind(checked_i64(batch_no, "batch_no")?)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(batch_commit_record_from_row(&row)?))
    }

    async fn list_batch_commits(
        &self,
        statuses: &[BatchCommitStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchCommitRecord>, StorageError> {
        let statuses = statuses
            .iter()
            .map(|status| status.as_str().to_owned())
            .collect::<Vec<_>>();
        let rows = sqlx::query(
            r#"
            SELECT batch_no, block_height, block_hash, status, attempts,
                   message_hash, message_hash_norm, last_error
            FROM l1_batch_commits
            WHERE status = ANY($1) AND attempts < $2
            ORDER BY batch_no ASC
            LIMIT $3
            "#,
        )
        .bind(statuses)
        .bind(checked_i32(max_attempts as usize, "max_attempts")?)
        .bind(checked_i32(limit as usize, "limit")?)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(batch_commit_record_from_row).collect()
    }

    async fn save_batch_commit(&self, record: BatchCommitRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO l1_batch_commits (
                batch_no, block_height, block_hash, status, attempts,
                message_hash, message_hash_norm, last_error
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (batch_no) DO UPDATE SET
                block_height = EXCLUDED.block_height,
                block_hash = EXCLUDED.block_hash,
                status = EXCLUDED.status,
                attempts = EXCLUDED.attempts,
                message_hash = EXCLUDED.message_hash,
                message_hash_norm = EXCLUDED.message_hash_norm,
                last_error = EXCLUDED.last_error,
                updated_at = now()
            "#,
        )
        .bind(checked_i64(record.batch_no, "batch_no")?)
        .bind(checked_i64(record.block_height, "block_height")?)
        .bind(record.block_hash.to_hex())
        .bind(record.status.as_str())
        .bind(
            i32::try_from(record.attempts).map_err(|_| StorageError::BigIntOverflow {
                field: "attempts",
                value: u64::from(record.attempts),
            })?,
        )
        .bind(record.message_hash.map(Hash32::to_hex))
        .bind(record.message_hash_norm.map(Hash32::to_hex))
        .bind(record.last_error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn checked_i64(value: u64, field: &'static str) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::BigIntOverflow { field, value })
}

fn checked_i32(value: usize, field: &'static str) -> Result<i32, StorageError> {
    i32::try_from(value).map_err(|_| StorageError::BigIntOverflow {
        field,
        value: value as u64,
    })
}

fn batch_commit_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<BatchCommitRecord, StorageError> {
    let batch_no: i64 = row.try_get("batch_no")?;
    let block_height: i64 = row.try_get("block_height")?;
    let block_hash: String = row.try_get("block_hash")?;
    let status: String = row.try_get("status")?;
    let attempts: i32 = row.try_get("attempts")?;
    let message_hash: Option<String> = row.try_get("message_hash")?;
    let message_hash_norm: Option<String> = row.try_get("message_hash_norm")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(BatchCommitRecord {
        batch_no: batch_no as u64,
        block_height: block_height as u64,
        block_hash: parse_hash("l1_batch_commits.block_hash", block_hash)?,
        status: BatchCommitStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "l1_batch_commits.status",
            value: status,
        })?,
        attempts: attempts as u32,
        message_hash: message_hash
            .map(|value| parse_hash("l1_batch_commits.message_hash", value))
            .transpose()?,
        message_hash_norm: message_hash_norm
            .map(|value| parse_hash("l1_batch_commits.message_hash_norm", value))
            .transpose()?,
        last_error,
    })
}

fn parse_hash(field: &'static str, value: String) -> Result<Hash32, StorageError> {
    Hash32::from_hex(&value).map_err(|_| StorageError::InvalidHash { field, value })
}
