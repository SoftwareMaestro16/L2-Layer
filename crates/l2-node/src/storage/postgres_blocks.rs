use l2_core::{Hash32, L2Block, WithdrawalProof};
use sqlx::Row;

use super::{BatchCommitRecord, PostgresStorage, StorageError, StoredTransaction};
use crate::storage::postgres_utils::{checked_i32, checked_i64};

pub(super) async fn save_block(
    storage: &PostgresStorage,
    block: L2Block,
) -> Result<(), StorageError> {
    let block_height = checked_i64(block.header.height, "block_height")?;
    let block_hash = block.header.block_hash().to_hex();
    let block_json = serde_json::to_value(&block)?;
    let mut tx = storage.pool.begin().await?;

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
        .bind(checked_i32(index, "tx_index")?)
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

pub(super) async fn get_block(
    storage: &PostgresStorage,
    height: u64,
) -> Result<Option<L2Block>, StorageError> {
    let Some(value) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT block_json FROM l2_blocks WHERE height = $1",
    )
    .bind(checked_i64(height, "block_height")?)
    .fetch_optional(&storage.pool)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_value(value)?))
}

pub(super) async fn get_transaction(
    storage: &PostgresStorage,
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
    .fetch_optional(&storage.pool)
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

pub(super) async fn get_withdrawal_proof(
    storage: &PostgresStorage,
    withdrawal_id: Hash32,
) -> Result<Option<WithdrawalProof>, StorageError> {
    let Some(height) = sqlx::query_scalar::<_, i64>(
        "SELECT block_height FROM l2_withdrawals WHERE withdrawal_id = $1",
    )
    .bind(withdrawal_id.to_hex())
    .fetch_optional(&storage.pool)
    .await?
    else {
        return Ok(None);
    };
    let Some(block) = get_block(storage, height as u64).await? else {
        return Ok(None);
    };
    Ok(block.withdrawal_proof(withdrawal_id))
}
