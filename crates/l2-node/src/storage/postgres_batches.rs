use l2_core::Hash32;
use sqlx::Row;

use super::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationStatus, PostgresStorage, StorageError,
    StoredBatchPayload,
};
use crate::storage::postgres_utils::{checked_i32, checked_i64, parse_hash};

const BATCH_COMMIT_COLUMNS: &str = r#"
    batch_no, block_height, block_hash, status, attempts,
    message_hash, message_hash_norm, last_error,
    l1_committed_at, finalization_eligible_at,
    finalization_status, finalization_attempts,
    finalize_message_hash, finalize_message_hash_norm,
    finalization_last_error
"#;

pub(super) async fn get_batch_commit(
    storage: &PostgresStorage,
    batch_no: u64,
) -> Result<Option<BatchCommitRecord>, StorageError> {
    let query = format!("SELECT {BATCH_COMMIT_COLUMNS} FROM l1_batch_commits WHERE batch_no = $1");
    let Some(row) = sqlx::query(&query)
        .bind(checked_i64(batch_no, "batch_no")?)
        .fetch_optional(&storage.pool)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(batch_commit_record_from_row(&row)?))
}

pub(super) async fn list_batch_commits(
    storage: &PostgresStorage,
    statuses: &[BatchCommitStatus],
    max_attempts: u32,
    limit: u32,
) -> Result<Vec<BatchCommitRecord>, StorageError> {
    let statuses = statuses
        .iter()
        .map(|status| status.as_str().to_owned())
        .collect::<Vec<_>>();
    let query = format!(
        r#"
        SELECT {BATCH_COMMIT_COLUMNS}
        FROM l1_batch_commits
        WHERE status = ANY($1) AND attempts < $2
        ORDER BY batch_no ASC
        LIMIT $3
        "#
    );
    let rows = sqlx::query(&query)
        .bind(statuses)
        .bind(checked_i32(max_attempts as usize, "max_attempts")?)
        .bind(checked_i32(limit as usize, "limit")?)
        .fetch_all(&storage.pool)
        .await?;
    rows.iter().map(batch_commit_record_from_row).collect()
}

pub(super) async fn save_batch_commit(
    storage: &PostgresStorage,
    record: BatchCommitRecord,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO l1_batch_commits (
            batch_no, block_height, block_hash, status, attempts,
            message_hash, message_hash_norm, last_error,
            l1_committed_at, finalization_eligible_at,
            finalization_status, finalization_attempts,
            finalize_message_hash, finalize_message_hash_norm,
            finalization_last_error
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (batch_no) DO UPDATE SET
            block_height = EXCLUDED.block_height,
            block_hash = EXCLUDED.block_hash,
            status = EXCLUDED.status,
            attempts = EXCLUDED.attempts,
            message_hash = EXCLUDED.message_hash,
            message_hash_norm = EXCLUDED.message_hash_norm,
            last_error = EXCLUDED.last_error,
            l1_committed_at = EXCLUDED.l1_committed_at,
            finalization_eligible_at = EXCLUDED.finalization_eligible_at,
            finalization_status = EXCLUDED.finalization_status,
            finalization_attempts = EXCLUDED.finalization_attempts,
            finalize_message_hash = EXCLUDED.finalize_message_hash,
            finalize_message_hash_norm = EXCLUDED.finalize_message_hash_norm,
            finalization_last_error = EXCLUDED.finalization_last_error,
            updated_at = now()
        "#,
    )
    .bind(checked_i64(record.batch_no, "batch_no")?)
    .bind(checked_i64(record.block_height, "block_height")?)
    .bind(record.block_hash.to_hex())
    .bind(record.status.as_str())
    .bind(checked_i32(record.attempts as usize, "attempts")?)
    .bind(record.message_hash.map(Hash32::to_hex))
    .bind(record.message_hash_norm.map(Hash32::to_hex))
    .bind(record.last_error)
    .bind(
        record
            .l1_committed_at
            .map(|value| checked_i64(value, "l1_committed_at"))
            .transpose()?,
    )
    .bind(
        record
            .finalization_eligible_at
            .map(|value| checked_i64(value, "finalization_eligible_at"))
            .transpose()?,
    )
    .bind(record.finalization_status.as_str())
    .bind(checked_i32(
        record.finalization_attempts as usize,
        "finalization_attempts",
    )?)
    .bind(record.finalize_message_hash.map(Hash32::to_hex))
    .bind(record.finalize_message_hash_norm.map(Hash32::to_hex))
    .bind(record.finalization_last_error)
    .execute(&storage.pool)
    .await?;
    Ok(())
}

pub(super) async fn save_batch_payload(
    storage: &PostgresStorage,
    payload: StoredBatchPayload,
) -> Result<bool, StorageError> {
    let inserted = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO l2_batch_payloads (
            block_height, block_hash, data_hash, payload_bytes, payload_size
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (block_height) DO NOTHING
        RETURNING block_height
        "#,
    )
    .bind(checked_i64(payload.block_height, "block_height")?)
    .bind(payload.block_hash.to_hex())
    .bind(payload.data_hash.to_hex())
    .bind(payload.payload_bytes.clone())
    .bind(checked_i64(
        payload.payload_bytes.len() as u64,
        "payload_size",
    )?)
    .fetch_optional(&storage.pool)
    .await?;
    if inserted.is_some() {
        return Ok(true);
    }
    let Some(existing) = get_batch_payload(storage, payload.block_height).await? else {
        return Err(StorageError::Conflict {
            resource: "batch payload",
        });
    };
    if existing == payload {
        Ok(false)
    } else {
        Err(StorageError::Conflict {
            resource: "batch payload",
        })
    }
}

pub(super) async fn get_batch_payload(
    storage: &PostgresStorage,
    block_height: u64,
) -> Result<Option<StoredBatchPayload>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT block_height, block_hash, data_hash, payload_bytes
        FROM l2_batch_payloads
        WHERE block_height = $1
        "#,
    )
    .bind(checked_i64(block_height, "block_height")?)
    .fetch_optional(&storage.pool)
    .await?
    else {
        return Ok(None);
    };
    let block_height: i64 = row.try_get("block_height")?;
    let block_hash: String = row.try_get("block_hash")?;
    let data_hash: String = row.try_get("data_hash")?;
    let payload_bytes: Vec<u8> = row.try_get("payload_bytes")?;
    Ok(Some(StoredBatchPayload {
        block_height: block_height as u64,
        block_hash: parse_hash("l2_batch_payloads.block_hash", block_hash)?,
        data_hash: parse_hash("l2_batch_payloads.data_hash", data_hash)?,
        payload_bytes,
    }))
}

fn batch_commit_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<BatchCommitRecord, StorageError> {
    let status: String = row.try_get("status")?;
    let finalization_status: String = row.try_get("finalization_status")?;
    Ok(BatchCommitRecord {
        batch_no: row.try_get::<i64, _>("batch_no")? as u64,
        block_height: row.try_get::<i64, _>("block_height")? as u64,
        block_hash: parse_hash("l1_batch_commits.block_hash", row.try_get("block_hash")?)?,
        status: BatchCommitStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "l1_batch_commits.status",
            value: status,
        })?,
        attempts: row.try_get::<i32, _>("attempts")? as u32,
        message_hash: parse_optional_hash(row, "message_hash", "l1_batch_commits.message_hash")?,
        message_hash_norm: parse_optional_hash(
            row,
            "message_hash_norm",
            "l1_batch_commits.message_hash_norm",
        )?,
        last_error: row.try_get("last_error")?,
        l1_committed_at: row
            .try_get::<Option<i64>, _>("l1_committed_at")?
            .map(|value| value as u64),
        finalization_eligible_at: row
            .try_get::<Option<i64>, _>("finalization_eligible_at")?
            .map(|value| value as u64),
        finalization_status: BatchFinalizationStatus::parse(&finalization_status).ok_or(
            StorageError::InvalidStatus {
                field: "l1_batch_commits.finalization_status",
                value: finalization_status,
            },
        )?,
        finalization_attempts: row.try_get::<i32, _>("finalization_attempts")? as u32,
        finalize_message_hash: parse_optional_hash(
            row,
            "finalize_message_hash",
            "l1_batch_commits.finalize_message_hash",
        )?,
        finalize_message_hash_norm: parse_optional_hash(
            row,
            "finalize_message_hash_norm",
            "l1_batch_commits.finalize_message_hash_norm",
        )?,
        finalization_last_error: row.try_get("finalization_last_error")?,
    })
}

fn parse_optional_hash(
    row: &sqlx::postgres::PgRow,
    column: &'static str,
    field: &'static str,
) -> Result<Option<Hash32>, StorageError> {
    row.try_get::<Option<String>, _>(column)?
        .map(|value| parse_hash(field, value))
        .transpose()
}
