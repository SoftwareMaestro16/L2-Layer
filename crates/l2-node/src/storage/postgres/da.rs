use super::{StorageError, StoredBatchPayload};
use crate::storage::postgres_util::{checked_i64, parse_hash};
use sqlx::postgres::PgPool;
use sqlx::Row;

pub(super) async fn save_batch_payload(
    pool: &PgPool,
    payload: StoredBatchPayload,
) -> Result<bool, StorageError> {
    let inserted = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO l2_batch_payloads (
            block_height,
            block_hash,
            data_hash,
            payload_bytes,
            payload_size,
            public_ref,
            public_uri
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
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
    .bind(payload.public_ref.clone())
    .bind(payload.public_uri.clone())
    .fetch_optional(pool)
    .await?;

    if inserted.is_some() {
        return Ok(true);
    }

    let Some(existing) = get_batch_payload(pool, payload.block_height).await? else {
        return Err(StorageError::Conflict {
            resource: "batch payload",
        });
    };
    let mut merged = existing;
    if merged.has_same_canonical_payload(&payload) && !merged.has_public_ref_conflict(&payload) {
        if merged.merge_public_metadata_from(&payload) {
            sqlx::query(
                r#"
                UPDATE l2_batch_payloads
                SET public_ref = $2, public_uri = $3
                WHERE block_height = $1
                "#,
            )
            .bind(checked_i64(merged.block_height, "block_height")?)
            .bind(merged.public_ref.clone())
            .bind(merged.public_uri.clone())
            .execute(pool)
            .await?;
        }
        Ok(false)
    } else {
        Err(StorageError::Conflict {
            resource: "batch payload",
        })
    }
}

pub(super) async fn get_batch_payload(
    pool: &PgPool,
    block_height: u64,
) -> Result<Option<StoredBatchPayload>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT block_height, block_hash, data_hash, payload_bytes, public_ref, public_uri
        FROM l2_batch_payloads
        WHERE block_height = $1
        "#,
    )
    .bind(checked_i64(block_height, "block_height")?)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let block_height: i64 = row.try_get("block_height")?;
    let block_hash: String = row.try_get("block_hash")?;
    let data_hash: String = row.try_get("data_hash")?;
    let payload_bytes: Vec<u8> = row.try_get("payload_bytes")?;
    let public_ref: Option<String> = row.try_get("public_ref")?;
    let public_uri: Option<String> = row.try_get("public_uri")?;

    Ok(Some(StoredBatchPayload {
        block_height: block_height as u64,
        block_hash: parse_hash("l2_batch_payloads.block_hash", block_hash)?,
        data_hash: parse_hash("l2_batch_payloads.data_hash", data_hash)?,
        payload_bytes,
        public_ref,
        public_uri,
    }))
}
