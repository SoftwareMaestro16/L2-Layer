use l2_core::{DepositEvent, Hash32};
use sqlx::postgres::{PgPool, PgRow};
use sqlx::Row;

use super::postgres_util::{checked_i32, checked_i64, parse_hash};
use super::{
    EntFaucetClaimRecord, EntFaucetClaimSaveResult, EntFaucetClaimSaveStatus, EntFaucetClaimStatus,
    StorageError,
};

pub(super) async fn save_ent_faucet_grant(
    pool: &PgPool,
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
    .fetch_optional(pool)
    .await?;

    Ok(inserted.is_some())
}

pub(super) async fn save_ent_faucet_batch_claim(
    pool: &PgPool,
    mut record: EntFaucetClaimRecord,
    deposit: DepositEvent,
) -> Result<EntFaucetClaimSaveResult, StorageError> {
    let mut tx = pool.begin().await?;
    if let Some(existing) = fetch_claim_by_id(&mut tx, &record.claim_id).await? {
        tx.commit().await?;
        return Ok(EntFaucetClaimSaveResult {
            status: EntFaucetClaimSaveStatus::DuplicateClaim,
            record: existing,
        });
    }

    if account_has_faucet_grant(&mut tx, record.account_id).await? {
        record.status = EntFaucetClaimStatus::DuplicateAccount;
        insert_faucet_claim(&mut tx, &record).await?;
        tx.commit().await?;
        return Ok(EntFaucetClaimSaveResult {
            status: EntFaucetClaimSaveStatus::DuplicateAccount,
            record,
        });
    }

    record.status = EntFaucetClaimStatus::Granted;
    insert_ent_faucet_grant(&mut tx, record.account_id, record.amount_base_units).await?;
    insert_deposit(&mut tx, deposit).await?;
    insert_faucet_claim(&mut tx, &record).await?;
    tx.commit().await?;

    Ok(EntFaucetClaimSaveResult {
        status: EntFaucetClaimSaveStatus::Granted,
        record,
    })
}

pub(super) async fn list_ent_faucet_claims(
    pool: &PgPool,
    limit: u32,
) -> Result<Vec<EntFaucetClaimRecord>, StorageError> {
    let rows = sqlx::query(
        r#"
        SELECT batch_id, claim_index, claim_id, account_id, amount::TEXT,
               deposit_id, status
        FROM ent_faucet_claims
        ORDER BY created_at DESC, claim_id DESC
        LIMIT $1
        "#,
    )
    .bind(checked_i32(limit as usize, "limit")?)
    .fetch_all(pool)
    .await?;

    rows.iter().map(faucet_claim_record_from_row).collect()
}

async fn fetch_claim_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim_id: &str,
) -> Result<Option<EntFaucetClaimRecord>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT batch_id, claim_index, claim_id, account_id, amount::TEXT,
               deposit_id, status
        FROM ent_faucet_claims
        WHERE claim_id = $1
        "#,
    )
    .bind(claim_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };

    faucet_claim_record_from_row(&row).map(Some)
}

async fn account_has_faucet_grant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: Hash32,
) -> Result<bool, StorageError> {
    let account = sqlx::query_scalar::<_, String>(
        "SELECT account_id FROM ent_faucet_grants WHERE account_id = $1",
    )
    .bind(account_id.to_hex())
    .fetch_optional(&mut **tx)
    .await?;
    Ok(account.is_some())
}

async fn insert_ent_faucet_grant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: Hash32,
    amount: u128,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO ent_faucet_grants (account_id, asset_id, amount)
        VALUES ($1, 0, $2::numeric)
        "#,
    )
    .bind(account_id.to_hex())
    .bind(amount.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_deposit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    deposit: DepositEvent,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO l2_deposits (
            deposit_id, asset_id, recipient, amount, l1_tx_hash, l1_lt, deposit_json
        )
        VALUES ($1, $2, $3, $4::numeric, $5, $6, $7)
        "#,
    )
    .bind(deposit.deposit_id.to_hex())
    .bind(i64::from(deposit.asset_id))
    .bind(deposit.recipient.to_hex())
    .bind(deposit.amount.to_string())
    .bind(deposit.l1_tx_hash.to_hex())
    .bind(checked_i64(deposit.l1_lt, "l1_lt")?)
    .bind(serde_json::to_value(deposit)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_faucet_claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    record: &EntFaucetClaimRecord,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO ent_faucet_claims (
            batch_id, claim_index, claim_id, account_id, asset_id, amount, deposit_id, status
        )
        VALUES ($1, $2, $3, $4, 0, $5::numeric, $6, $7)
        "#,
    )
    .bind(record.batch_id.to_hex())
    .bind(checked_i32(record.claim_index as usize, "claim_index")?)
    .bind(&record.claim_id)
    .bind(record.account_id.to_hex())
    .bind(record.amount_base_units.to_string())
    .bind(record.deposit_id.to_hex())
    .bind(record.status.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn faucet_claim_record_from_row(row: &PgRow) -> Result<EntFaucetClaimRecord, StorageError> {
    let batch_id: String = row.try_get("batch_id")?;
    let claim_index: i32 = row.try_get("claim_index")?;
    let claim_id: String = row.try_get("claim_id")?;
    let account_id: String = row.try_get("account_id")?;
    let amount: String = row.try_get("amount")?;
    let deposit_id: String = row.try_get("deposit_id")?;
    let status: String = row.try_get("status")?;

    Ok(EntFaucetClaimRecord {
        batch_id: parse_hash("ent_faucet_claims.batch_id", batch_id)?,
        claim_index: claim_index as u32,
        claim_id,
        account_id: parse_hash("ent_faucet_claims.account_id", account_id)?,
        amount_base_units: amount.parse().map_err(|_| StorageError::InvalidNumeric {
            field: "ent_faucet_claims.amount",
            value: amount,
        })?,
        deposit_id: parse_hash("ent_faucet_claims.deposit_id", deposit_id)?,
        status: EntFaucetClaimStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "ent_faucet_claims.status",
            value: status,
        })?,
    })
}
