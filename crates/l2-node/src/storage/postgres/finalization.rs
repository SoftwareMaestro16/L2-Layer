use super::{
    postgres_util::{batch_finalization_record_from_row, checked_i32, checked_i64},
    BatchFinalizationRecord, BatchFinalizationStatus, StorageError,
};
use l2_core::Hash32;
use sqlx::postgres::PgPool;

pub(super) async fn get_batch_finalization(
    pool: &PgPool,
    batch_no: u64,
) -> Result<Option<BatchFinalizationRecord>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT batch_no, block_height, status, attempts, finalize_after_unix,
               message_hash, message_hash_norm, last_error
        FROM l1_batch_finalizations
        WHERE batch_no = $1
        "#,
    )
    .bind(checked_i64(batch_no, "batch_no")?)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    Ok(Some(batch_finalization_record_from_row(&row)?))
}

pub(super) async fn list_batch_finalizations(
    pool: &PgPool,
    statuses: &[BatchFinalizationStatus],
    max_attempts: u32,
    limit: u32,
) -> Result<Vec<BatchFinalizationRecord>, StorageError> {
    let statuses = statuses
        .iter()
        .map(|status| status.as_str().to_owned())
        .collect::<Vec<_>>();
    let rows = sqlx::query(
        r#"
        SELECT batch_no, block_height, status, attempts, finalize_after_unix,
               message_hash, message_hash_norm, last_error
        FROM l1_batch_finalizations
        WHERE status = ANY($1) AND attempts < $2
        ORDER BY batch_no ASC
        LIMIT $3
        "#,
    )
    .bind(statuses)
    .bind(checked_i32(max_attempts as usize, "max_attempts")?)
    .bind(checked_i32(limit as usize, "limit")?)
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(batch_finalization_record_from_row)
        .collect()
}

pub(super) async fn latest_batch_finalization(
    pool: &PgPool,
    statuses: &[BatchFinalizationStatus],
) -> Result<Option<BatchFinalizationRecord>, StorageError> {
    let statuses = statuses
        .iter()
        .map(|status| status.as_str().to_owned())
        .collect::<Vec<_>>();
    let row = if statuses.is_empty() {
        sqlx::query(
            r#"
            SELECT batch_no, block_height, status, attempts, finalize_after_unix,
                   message_hash, message_hash_norm, last_error
            FROM l1_batch_finalizations
            ORDER BY batch_no DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT batch_no, block_height, status, attempts, finalize_after_unix,
                   message_hash, message_hash_norm, last_error
            FROM l1_batch_finalizations
            WHERE status = ANY($1)
            ORDER BY batch_no DESC
            LIMIT 1
            "#,
        )
        .bind(statuses)
        .fetch_optional(pool)
        .await?
    };

    row.as_ref()
        .map(batch_finalization_record_from_row)
        .transpose()
}

pub(super) async fn save_batch_finalization(
    pool: &PgPool,
    record: BatchFinalizationRecord,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO l1_batch_finalizations (
            batch_no, block_height, status, attempts, finalize_after_unix,
            message_hash, message_hash_norm, last_error
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (batch_no) DO UPDATE SET
            block_height = EXCLUDED.block_height,
            status = EXCLUDED.status,
            attempts = EXCLUDED.attempts,
            finalize_after_unix = EXCLUDED.finalize_after_unix,
            message_hash = EXCLUDED.message_hash,
            message_hash_norm = EXCLUDED.message_hash_norm,
            last_error = EXCLUDED.last_error,
            updated_at = now()
        "#,
    )
    .bind(checked_i64(record.batch_no, "batch_no")?)
    .bind(checked_i64(record.block_height, "block_height")?)
    .bind(record.status.as_str())
    .bind(
        i32::try_from(record.attempts).map_err(|_| StorageError::BigIntOverflow {
            field: "attempts",
            value: u64::from(record.attempts),
        })?,
    )
    .bind(checked_i64(
        record.finalize_after_unix,
        "finalize_after_unix",
    )?)
    .bind(record.message_hash.map(Hash32::to_hex))
    .bind(record.message_hash_norm.map(Hash32::to_hex))
    .bind(record.last_error)
    .execute(pool)
    .await?;

    Ok(())
}
