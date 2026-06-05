use async_trait::async_trait;
use l2_core::{DepositEvent, Hash32, L2Block, WithdrawalProof};
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;

use super::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus,
    EntFaucetClaimRecord, EntFaucetClaimSaveResult, ExplorerStorageStats,
    InternalQueueSnapshotRecord, L1Cursor, ObserverCheckpoint, Storage, StorageError,
    StoredBatchPayload, StoredContractState, StoredTransaction, VerifierSourceFile, VerifierStatus,
    VerifierSubmissionRecord,
};
use crate::storage::postgres_util::{batch_commit_record_from_row, checked_i32, checked_i64};
use crate::storage::{
    postgres_contracts, postgres_da, postgres_faucet, postgres_finalization,
    postgres_internal_queue, postgres_observer,
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
    async fn health_check(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

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

        sqlx::query("DELETE FROM l2_transactions WHERE block_height = $1")
            .bind(block_height)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM l2_withdrawals WHERE block_height = $1")
            .bind(block_height)
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
            SELECT t.block_height, t.tx_index, t.tx_json, t.receipt_json, b.block_json
            FROM l2_transactions t
            JOIN l2_blocks b ON b.height = t.block_height
            WHERE t.tx_hash = $1
            "#,
        )
        .bind(hash.to_hex())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        let block_height: i64 = row.try_get("block_height")?;
        let tx_index: i32 = row.try_get("tx_index")?;
        let transaction: serde_json::Value = row.try_get("tx_json")?;
        let receipt: Option<serde_json::Value> = row.try_get("receipt_json")?;
        let block: L2Block = serde_json::from_value(row.try_get("block_json")?)?;
        Ok(Some(StoredTransaction {
            block_height: block_height as u64,
            block_timestamp: block.header.timestamp,
            block_hash: block.header.block_hash(),
            tx_index: tx_index as usize,
            transaction: serde_json::from_value(transaction)?,
            receipt: receipt.map(serde_json::from_value).transpose()?,
        }))
    }

    async fn explorer_storage_stats(&self) -> Result<ExplorerStorageStats, StorageError> {
        let block_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM l2_blocks")
            .fetch_one(&self.pool)
            .await?;
        let transaction_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM l2_transactions")
            .fetch_one(&self.pool)
            .await?;
        let deposit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM l2_transactions WHERE tx_json #>> '{kind,Deposit,deposit_id}' IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        let withdrawal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM l2_withdrawals")
            .fetch_one(&self.pool)
            .await?;
        Ok(ExplorerStorageStats {
            block_count: block_count as u64,
            transaction_count: transaction_count as u64,
            deposit_count: deposit_count as u64,
            withdrawal_count: withdrawal_count as u64,
        })
    }

    async fn list_account_transactions(
        &self,
        account_id: Hash32,
        before_height: Option<u64>,
        before_index: Option<usize>,
        limit: usize,
    ) -> Result<Vec<StoredTransaction>, StorageError> {
        let account_id = account_id.to_hex();
        let before_height = before_height
            .map(|height| checked_i64(height, "before_height"))
            .transpose()?;
        let before_index = checked_i32(before_index.unwrap_or(i32::MAX as usize), "before_index")?;
        let rows = sqlx::query(
            r#"
            SELECT t.block_height, t.tx_index, t.tx_json, t.receipt_json, b.block_json
            FROM l2_transactions t
            JOIN l2_blocks b ON b.height = t.block_height
            WHERE (
                t.tx_json ->> 'from' = $1
                OR t.tx_json #>> '{kind,Deposit,recipient}' = $1
                OR t.tx_json #>> '{kind,Transfer,to}' = $1
                OR t.tx_json #>> '{kind,DeployContract,contract}' = $1
                OR t.tx_json #>> '{kind,CallContract,contract}' = $1
                OR t.tx_json #>> '{kind,InternalMessage,from}' = $1
                OR t.tx_json #>> '{kind,InternalMessage,to}' = $1
            )
            AND (
                $2::BIGINT IS NULL
                OR t.block_height < $2
                OR (t.block_height = $2 AND t.tx_index < $3)
            )
            ORDER BY t.block_height DESC, t.tx_index DESC
            LIMIT $4
            "#,
        )
        .bind(account_id)
        .bind(before_height)
        .bind(before_index)
        .bind(checked_i32(limit, "limit")?)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let block_height: i64 = row.try_get("block_height")?;
                let tx_index: i32 = row.try_get("tx_index")?;
                let transaction: serde_json::Value = row.try_get("tx_json")?;
                let receipt: Option<serde_json::Value> = row.try_get("receipt_json")?;
                let block: L2Block = serde_json::from_value(row.try_get("block_json")?)?;
                Ok(StoredTransaction {
                    block_height: block_height as u64,
                    block_timestamp: block.header.timestamp,
                    block_hash: block.header.block_hash(),
                    tx_index: tx_index as usize,
                    transaction: serde_json::from_value(transaction)?,
                    receipt: receipt.map(serde_json::from_value).transpose()?,
                })
            })
            .collect()
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
        postgres_faucet::save_ent_faucet_grant(&self.pool, account_id, amount).await
    }

    async fn save_ent_faucet_batch_claim(
        &self,
        record: EntFaucetClaimRecord,
        deposit: DepositEvent,
    ) -> Result<EntFaucetClaimSaveResult, StorageError> {
        postgres_faucet::save_ent_faucet_batch_claim(&self.pool, record, deposit).await
    }

    async fn list_ent_faucet_claims(
        &self,
        limit: u32,
    ) -> Result<Vec<EntFaucetClaimRecord>, StorageError> {
        postgres_faucet::list_ent_faucet_claims(&self.pool, limit).await
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
        let result = sqlx::query(
            r#"
            INSERT INTO l1_cursors (source, lt, hash)
            VALUES ($1, $2, $3)
            ON CONFLICT (source) DO UPDATE SET
                lt = EXCLUDED.lt,
                hash = EXCLUDED.hash,
                updated_at = now()
            WHERE l1_cursors.lt < EXCLUDED.lt
               OR (l1_cursors.lt = EXCLUDED.lt AND l1_cursors.hash = EXCLUDED.hash)
            "#,
        )
        .bind(source)
        .bind(checked_i64(cursor.lt, "cursor_lt")?)
        .bind(cursor.hash.to_hex())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StorageError::Conflict {
                resource: "l1 cursor",
            });
        }
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

    async fn latest_batch_commit(
        &self,
        statuses: &[BatchCommitStatus],
    ) -> Result<Option<BatchCommitRecord>, StorageError> {
        let statuses = statuses
            .iter()
            .map(|status| status.as_str().to_owned())
            .collect::<Vec<_>>();
        let row = if statuses.is_empty() {
            sqlx::query(
                r#"
                SELECT batch_no, block_height, block_hash, status, attempts,
                       message_hash, message_hash_norm, last_error
                FROM l1_batch_commits
                ORDER BY batch_no DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT batch_no, block_height, block_hash, status, attempts,
                       message_hash, message_hash_norm, last_error
                FROM l1_batch_commits
                WHERE status = ANY($1)
                ORDER BY batch_no DESC
                LIMIT 1
                "#,
            )
            .bind(statuses)
            .fetch_optional(&self.pool)
            .await?
        };

        row.as_ref().map(batch_commit_record_from_row).transpose()
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

    async fn get_batch_finalization(
        &self,
        batch_no: u64,
    ) -> Result<Option<BatchFinalizationRecord>, StorageError> {
        postgres_finalization::get_batch_finalization(&self.pool, batch_no).await
    }

    async fn list_batch_finalizations(
        &self,
        statuses: &[BatchFinalizationStatus],
        max_attempts: u32,
        limit: u32,
    ) -> Result<Vec<BatchFinalizationRecord>, StorageError> {
        postgres_finalization::list_batch_finalizations(&self.pool, statuses, max_attempts, limit)
            .await
    }

    async fn latest_batch_finalization(
        &self,
        statuses: &[BatchFinalizationStatus],
    ) -> Result<Option<BatchFinalizationRecord>, StorageError> {
        postgres_finalization::latest_batch_finalization(&self.pool, statuses).await
    }

    async fn save_batch_finalization(
        &self,
        record: BatchFinalizationRecord,
    ) -> Result<(), StorageError> {
        postgres_finalization::save_batch_finalization(&self.pool, record).await
    }

    async fn save_batch_payload(&self, payload: StoredBatchPayload) -> Result<bool, StorageError> {
        postgres_da::save_batch_payload(&self.pool, payload).await
    }

    async fn get_batch_payload(
        &self,
        block_height: u64,
    ) -> Result<Option<StoredBatchPayload>, StorageError> {
        postgres_da::get_batch_payload(&self.pool, block_height).await
    }

    async fn save_contract_state(&self, record: StoredContractState) -> Result<(), StorageError> {
        postgres_contracts::save_contract_state(&self.pool, record).await
    }

    async fn get_contract_state(
        &self,
        account_id: Hash32,
    ) -> Result<Option<StoredContractState>, StorageError> {
        postgres_contracts::get_contract_state(&self.pool, account_id).await
    }

    async fn save_verifier_submission(
        &self,
        submission: VerifierSubmissionRecord,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO contract_source_submissions (
                submission_id, code_hash, account_id, status, files_json
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (submission_id) DO NOTHING
            "#,
        )
        .bind(submission.submission_id.to_hex())
        .bind(submission.code_hash.to_hex())
        .bind(submission.account_id.map(Hash32::to_hex))
        .bind(submission.status.as_str())
        .bind(serde_json::to_value(&submission.files)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_verifier_source(
        &self,
        code_hash: Hash32,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError> {
        let Some(row) = sqlx::query(
            r#"
            SELECT submission_id, code_hash, account_id, status, files_json
            FROM contract_source_submissions
            WHERE code_hash = $1 AND status IN ('pending', 'verified')
            ORDER BY CASE status WHEN 'verified' THEN 2 WHEN 'pending' THEN 1 ELSE 0 END DESC,
                     created_at DESC
            LIMIT 1
            "#,
        )
        .bind(code_hash.to_hex())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        verifier_submission_from_row(row)
    }

    async fn review_verifier_submission(
        &self,
        submission_id: Hash32,
        status: VerifierStatus,
    ) -> Result<Option<VerifierSubmissionRecord>, StorageError> {
        let Some(row) = sqlx::query(
            r#"
            UPDATE contract_source_submissions
            SET status = $2, reviewed_at = now()
            WHERE submission_id = $1
            RETURNING submission_id, code_hash, account_id, status, files_json
            "#,
        )
        .bind(submission_id.to_hex())
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        verifier_submission_from_row(row)
    }

    async fn save_internal_queue_snapshot(
        &self,
        record: InternalQueueSnapshotRecord,
    ) -> Result<(), StorageError> {
        postgres_internal_queue::save_internal_queue_snapshot(&self.pool, record).await
    }

    async fn latest_internal_queue_snapshot(
        &self,
    ) -> Result<Option<InternalQueueSnapshotRecord>, StorageError> {
        postgres_internal_queue::latest_internal_queue_snapshot(&self.pool).await
    }

    async fn save_observer_checkpoint(
        &self,
        checkpoint: ObserverCheckpoint,
    ) -> Result<(), StorageError> {
        postgres_observer::save_observer_checkpoint(&self.pool, checkpoint).await
    }

    async fn latest_observer_checkpoint(&self) -> Result<Option<ObserverCheckpoint>, StorageError> {
        postgres_observer::latest_observer_checkpoint(&self.pool).await
    }
}

fn verifier_submission_from_row(
    row: PgRow,
) -> Result<Option<VerifierSubmissionRecord>, StorageError> {
    let submission_id: String = row.try_get("submission_id")?;
    let code_hash: String = row.try_get("code_hash")?;
    let account_id: Option<String> = row.try_get("account_id")?;
    let status: String = row.try_get("status")?;
    let files_json: serde_json::Value = row.try_get("files_json")?;
    Ok(Some(VerifierSubmissionRecord {
        submission_id: Hash32::from_hex(&submission_id).map_err(|_| StorageError::InvalidHash {
            field: "contract_source_submissions.submission_id",
            value: submission_id,
        })?,
        code_hash: Hash32::from_hex(&code_hash).map_err(|_| StorageError::InvalidHash {
            field: "contract_source_submissions.code_hash",
            value: code_hash,
        })?,
        account_id: account_id
            .map(|value| {
                Hash32::from_hex(&value).map_err(|_| StorageError::InvalidHash {
                    field: "contract_source_submissions.account_id",
                    value,
                })
            })
            .transpose()?,
        status: VerifierStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "contract_source_submissions.status",
            value: status,
        })?,
        files: serde_json::from_value::<Vec<VerifierSourceFile>>(files_json)?,
    }))
}
