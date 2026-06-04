use l2_core::{DepositEvent, Hash32};
use sqlx::Row;

use super::{L1Cursor, PostgresStorage, StorageError};
use crate::storage::postgres_utils::{checked_i64, parse_hash};

pub(super) async fn save_deposit(
    storage: &PostgresStorage,
    deposit: DepositEvent,
) -> Result<bool, StorageError> {
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
    .fetch_optional(&storage.pool)
    .await?;
    Ok(inserted.is_some())
}

pub(super) async fn save_ent_faucet_grant(
    storage: &PostgresStorage,
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
    .fetch_optional(&storage.pool)
    .await?;
    Ok(inserted.is_some())
}

pub(super) async fn get_l1_cursor(
    storage: &PostgresStorage,
    source: &str,
) -> Result<Option<L1Cursor>, StorageError> {
    let Some(row) = sqlx::query("SELECT lt, hash FROM l1_cursors WHERE source = $1")
        .bind(source)
        .fetch_optional(&storage.pool)
        .await?
    else {
        return Ok(None);
    };
    let lt: i64 = row.try_get("lt")?;
    let hash: String = row.try_get("hash")?;
    Ok(Some(L1Cursor {
        lt: lt as u64,
        hash: parse_hash("l1_cursors.hash", hash)?,
    }))
}

pub(super) async fn set_l1_cursor(
    storage: &PostgresStorage,
    source: &str,
    cursor: L1Cursor,
) -> Result<(), StorageError> {
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
    .execute(&storage.pool)
    .await?;
    Ok(())
}
