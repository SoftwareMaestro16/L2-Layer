use super::{ObserverCheckpoint, StorageError};
use crate::storage::postgres_util::{checked_i64, parse_hash};
use sqlx::postgres::PgPool;
use sqlx::Row;

pub(super) async fn save_observer_checkpoint(
    pool: &PgPool,
    checkpoint: ObserverCheckpoint,
) -> Result<(), StorageError> {
    if !checkpoint.validate_integrity() {
        return Err(StorageError::InvalidObserverCheckpoint {
            reason: "state root mismatch",
        });
    }
    sqlx::query(
        r#"
        INSERT INTO observer_checkpoints (
            next_batch_no,
            next_block_height,
            state_root,
            checkpoint_json
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (next_batch_no) DO UPDATE SET
            next_block_height = EXCLUDED.next_block_height,
            state_root = EXCLUDED.state_root,
            checkpoint_json = EXCLUDED.checkpoint_json
        "#,
    )
    .bind(checked_i64(checkpoint.next_batch_no, "next_batch_no")?)
    .bind(checked_i64(
        checkpoint.next_block_height,
        "next_block_height",
    )?)
    .bind(checkpoint.state_root.to_hex())
    .bind(serde_json::to_value(checkpoint)?)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) async fn latest_observer_checkpoint(
    pool: &PgPool,
) -> Result<Option<ObserverCheckpoint>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT state_root, checkpoint_json
        FROM observer_checkpoints
        ORDER BY next_batch_no DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let state_root: String = row.try_get("state_root")?;
    let mut checkpoint: ObserverCheckpoint =
        serde_json::from_value(row.try_get("checkpoint_json")?)?;
    checkpoint.state_root = parse_hash("observer_checkpoints.state_root", state_root)?;
    Ok(Some(checkpoint))
}
