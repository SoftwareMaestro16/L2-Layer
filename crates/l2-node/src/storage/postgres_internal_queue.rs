use super::{InternalQueueSnapshotRecord, StorageError};
use crate::storage::postgres_util::checked_i64;
use sqlx::postgres::PgPool;
use sqlx::Row;

pub(super) async fn save_internal_queue_snapshot(
    pool: &PgPool,
    record: InternalQueueSnapshotRecord,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO internal_message_queue_snapshots (
            block_height,
            queue_json
        )
        VALUES ($1, $2)
        ON CONFLICT (block_height) DO UPDATE SET
            queue_json = EXCLUDED.queue_json,
            updated_at = now()
        "#,
    )
    .bind(checked_i64(record.block_height, "block_height")?)
    .bind(serde_json::to_value(record)?)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn latest_internal_queue_snapshot(
    pool: &PgPool,
) -> Result<Option<InternalQueueSnapshotRecord>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT queue_json
        FROM internal_message_queue_snapshots
        ORDER BY block_height DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_value(row.try_get("queue_json")?)?))
}
