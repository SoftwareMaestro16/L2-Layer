use l2_core::{Account, Hash32};
use sqlx::postgres::PgPool;
use sqlx::Row;

use super::postgres_util::{checked_i32, checked_i64};
use super::{StorageError, StoredContractCodeCell, StoredContractDataCell, StoredContractState};

pub async fn save_contract_state(
    pool: &PgPool,
    record: StoredContractState,
) -> Result<(), StorageError> {
    let mut tx = pool.begin().await?;

    if let Some(existing) = sqlx::query(
        r#"
        SELECT code_boc_base64, size_bytes
        FROM contract_code_cells
        WHERE code_hash = $1
        "#,
    )
    .bind(record.code_cell.code_hash.to_hex())
    .fetch_optional(&mut *tx)
    .await?
    {
        let code_boc_base64: String = existing.try_get("code_boc_base64")?;
        let size_bytes: i32 = existing.try_get("size_bytes")?;
        if code_boc_base64 != record.code_cell.code_boc_base64
            || usize::try_from(size_bytes).unwrap_or_default() != record.code_cell.size_bytes
        {
            return Err(StorageError::Conflict {
                resource: "contract code cell",
            });
        }
    } else {
        sqlx::query(
            r#"
            INSERT INTO contract_code_cells (
                code_hash, code_boc_base64, size_bytes, first_seen_block_height
            )
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(record.code_cell.code_hash.to_hex())
        .bind(&record.code_cell.code_boc_base64)
        .bind(checked_i32(record.code_cell.size_bytes, "code_size_bytes")?)
        .bind(checked_i64(
            record.code_cell.first_seen_block_height,
            "first_seen_block_height",
        )?)
        .execute(&mut *tx)
        .await?;
    }

    if let Some(existing) = sqlx::query(
        r#"
        SELECT storage_root, data_boc_base64, size_bytes
        FROM contract_data_cells
        WHERE data_hash = $1
        "#,
    )
    .bind(record.data_cell.data_hash.to_hex())
    .fetch_optional(&mut *tx)
    .await?
    {
        let storage_root: String = existing.try_get("storage_root")?;
        let data_boc_base64: String = existing.try_get("data_boc_base64")?;
        let size_bytes: i32 = existing.try_get("size_bytes")?;
        if storage_root != record.data_cell.storage_root.to_hex()
            || data_boc_base64 != record.data_cell.data_boc_base64
            || usize::try_from(size_bytes).unwrap_or_default() != record.data_cell.size_bytes
        {
            return Err(StorageError::Conflict {
                resource: "contract data cell",
            });
        }
    } else {
        sqlx::query(
            r#"
            INSERT INTO contract_data_cells (
                data_hash, storage_root, data_boc_base64, size_bytes, first_seen_block_height
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(record.data_cell.data_hash.to_hex())
        .bind(record.data_cell.storage_root.to_hex())
        .bind(&record.data_cell.data_boc_base64)
        .bind(checked_i32(record.data_cell.size_bytes, "data_size_bytes")?)
        .bind(checked_i64(
            record.data_cell.first_seen_block_height,
            "first_seen_block_height",
        )?)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO contract_account_states (
            account_id, code_hash, data_hash, storage_root, last_block_height, account_json
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (account_id) DO UPDATE SET
            code_hash = EXCLUDED.code_hash,
            data_hash = EXCLUDED.data_hash,
            storage_root = EXCLUDED.storage_root,
            last_block_height = EXCLUDED.last_block_height,
            account_json = EXCLUDED.account_json,
            updated_at = now()
        WHERE contract_account_states.last_block_height <= EXCLUDED.last_block_height
        "#,
    )
    .bind(record.account_id.to_hex())
    .bind(record.account.code_hash.to_hex())
    .bind(record.account.data_hash.to_hex())
    .bind(record.account.storage_root.to_hex())
    .bind(checked_i64(record.last_block_height, "last_block_height")?)
    .bind(serde_json::to_value(&record.account)?)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_contract_state(
    pool: &PgPool,
    account_id: Hash32,
) -> Result<Option<StoredContractState>, StorageError> {
    let Some(row) = sqlx::query(
        r#"
        SELECT s.account_id, s.last_block_height, s.account_json,
               c.code_hash, c.code_boc_base64, c.size_bytes AS code_size_bytes,
               c.first_seen_block_height AS code_first_seen_block_height,
               d.data_hash, d.storage_root, d.data_boc_base64,
               d.size_bytes AS data_size_bytes,
               d.first_seen_block_height AS data_first_seen_block_height
        FROM contract_account_states s
        JOIN contract_code_cells c ON c.code_hash = s.code_hash
        JOIN contract_data_cells d ON d.data_hash = s.data_hash
        WHERE s.account_id = $1
        "#,
    )
    .bind(account_id.to_hex())
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let account: Account = serde_json::from_value(row.try_get("account_json")?)?;
    let code_hash: String = row.try_get("code_hash")?;
    let data_hash: String = row.try_get("data_hash")?;
    let storage_root: String = row.try_get("storage_root")?;
    let last_block_height: i64 = row.try_get("last_block_height")?;
    let code_size_bytes: i32 = row.try_get("code_size_bytes")?;
    let data_size_bytes: i32 = row.try_get("data_size_bytes")?;
    let code_first_seen_block_height: i64 = row.try_get("code_first_seen_block_height")?;
    let data_first_seen_block_height: i64 = row.try_get("data_first_seen_block_height")?;

    Ok(Some(StoredContractState {
        account_id,
        account,
        code_cell: StoredContractCodeCell {
            code_hash: parse_hash("contract_code_cells.code_hash", code_hash)?,
            code_boc_base64: row.try_get("code_boc_base64")?,
            size_bytes: code_size_bytes as usize,
            first_seen_block_height: code_first_seen_block_height as u64,
        },
        data_cell: StoredContractDataCell {
            data_hash: parse_hash("contract_data_cells.data_hash", data_hash)?,
            storage_root: parse_hash("contract_data_cells.storage_root", storage_root)?,
            data_boc_base64: row.try_get("data_boc_base64")?,
            size_bytes: data_size_bytes as usize,
            first_seen_block_height: data_first_seen_block_height as u64,
        },
        last_block_height: last_block_height as u64,
    }))
}

fn parse_hash(field: &'static str, value: String) -> Result<Hash32, StorageError> {
    Hash32::from_hex(&value).map_err(|_| StorageError::InvalidHash { field, value })
}
