use l2_core::Hash32;

use super::StorageError;

pub(super) fn checked_i64(value: u64, field: &'static str) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::BigIntOverflow { field, value })
}

pub(super) fn checked_i32(value: usize, field: &'static str) -> Result<i32, StorageError> {
    i32::try_from(value).map_err(|_| StorageError::BigIntOverflow {
        field,
        value: value as u64,
    })
}

pub(super) fn parse_hash(field: &'static str, value: String) -> Result<Hash32, StorageError> {
    Hash32::from_hex(&value).map_err(|_| StorageError::InvalidHash { field, value })
}
