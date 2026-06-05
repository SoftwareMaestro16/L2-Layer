use super::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus,
    StorageError,
};
use l2_core::Hash32;
use sqlx::Row;

pub(super) fn checked_i64(value: u64, field: &'static str) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::BigIntOverflow { field, value })
}

pub(super) fn checked_i32(value: usize, field: &'static str) -> Result<i32, StorageError> {
    i32::try_from(value).map_err(|_| StorageError::BigIntOverflow {
        field,
        value: value as u64,
    })
}

pub(super) fn batch_commit_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<BatchCommitRecord, StorageError> {
    let batch_no: i64 = row.try_get("batch_no")?;
    let block_height: i64 = row.try_get("block_height")?;
    let block_hash: String = row.try_get("block_hash")?;
    let status: String = row.try_get("status")?;
    let attempts: i32 = row.try_get("attempts")?;
    let message_hash: Option<String> = row.try_get("message_hash")?;
    let message_hash_norm: Option<String> = row.try_get("message_hash_norm")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(BatchCommitRecord {
        batch_no: batch_no as u64,
        block_height: block_height as u64,
        block_hash: parse_hash("l1_batch_commits.block_hash", block_hash)?,
        status: BatchCommitStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "l1_batch_commits.status",
            value: status,
        })?,
        attempts: attempts as u32,
        message_hash: message_hash
            .map(|value| parse_hash("l1_batch_commits.message_hash", value))
            .transpose()?,
        message_hash_norm: message_hash_norm
            .map(|value| parse_hash("l1_batch_commits.message_hash_norm", value))
            .transpose()?,
        last_error,
    })
}

pub(super) fn batch_finalization_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<BatchFinalizationRecord, StorageError> {
    let batch_no: i64 = row.try_get("batch_no")?;
    let block_height: i64 = row.try_get("block_height")?;
    let status: String = row.try_get("status")?;
    let attempts: i32 = row.try_get("attempts")?;
    let finalize_after_unix: i64 = row.try_get("finalize_after_unix")?;
    let message_hash: Option<String> = row.try_get("message_hash")?;
    let message_hash_norm: Option<String> = row.try_get("message_hash_norm")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(BatchFinalizationRecord {
        batch_no: batch_no as u64,
        block_height: block_height as u64,
        status: BatchFinalizationStatus::parse(&status).ok_or(StorageError::InvalidStatus {
            field: "l1_batch_finalizations.status",
            value: status,
        })?,
        attempts: attempts as u32,
        finalize_after_unix: finalize_after_unix as u64,
        message_hash: message_hash
            .map(|value| parse_hash("l1_batch_finalizations.message_hash", value))
            .transpose()?,
        message_hash_norm: message_hash_norm
            .map(|value| parse_hash("l1_batch_finalizations.message_hash_norm", value))
            .transpose()?,
        last_error,
    })
}

pub(super) fn parse_hash(field: &'static str, value: String) -> Result<Hash32, StorageError> {
    Hash32::from_hex(&value).map_err(|_| StorageError::InvalidHash { field, value })
}
