use super::{ApiError, AppState};
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{
    is_l2_zero_address, l2_raw_address, l2_user_friendly_address, parse_l2_address, AccountFlags,
    AccountRecoveryLock, AccountType, Hash32,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(in crate::api) struct AccountMetadata {
    pub(in crate::api) account_id: Hash32,
    pub(in crate::api) raw_address: String,
    pub(in crate::api) user_friendly_address: String,
    pub(in crate::api) status: &'static str,
    pub(in crate::api) account_type: AccountType,
    pub(in crate::api) flags: AccountFlags,
    pub(in crate::api) active_public_key: Option<Hash32>,
    pub(in crate::api) active_public_key_set: bool,
    pub(in crate::api) recovery_lock: Option<AccountRecoveryLock>,
    pub(in crate::api) nonce: u64,
    pub(in crate::api) code_hash: Hash32,
    pub(in crate::api) data_hash: Hash32,
    pub(in crate::api) storage_root: Hash32,
    pub(in crate::api) has_code: bool,
    pub(in crate::api) has_data: bool,
    pub(in crate::api) last_lt: u64,
}

pub(in crate::api) async fn get_account_metadata(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AccountMetadata>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    if is_l2_zero_address(id) {
        return Ok(Json(AccountMetadata {
            account_id: id,
            raw_address: l2_raw_address(id),
            user_friendly_address: l2_user_friendly_address(id),
            status: "reserved",
            account_type: AccountType::System,
            flags: AccountFlags {
                disabled: true,
                contract_only: false,
                system_only: true,
            },
            active_public_key: None,
            active_public_key_set: false,
            recovery_lock: None,
            nonce: 0,
            code_hash: Hash32::ZERO,
            data_hash: Hash32::ZERO,
            storage_root: Hash32::ZERO,
            has_code: false,
            has_data: false,
            last_lt: 0,
        }));
    }

    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    let active_public_key_set = account.active_public_key.is_some();

    Ok(Json(AccountMetadata {
        account_id: id,
        raw_address: l2_raw_address(id),
        user_friendly_address: l2_user_friendly_address(id),
        status: metadata_status(&account),
        account_type: account.account_type,
        flags: account.flags,
        active_public_key: account.active_public_key,
        active_public_key_set,
        recovery_lock: account.recovery_lock,
        nonce: account.nonce,
        code_hash: account.code_hash,
        data_hash: account.data_hash,
        storage_root: account.storage_root,
        has_code: account.code_hash != Hash32::ZERO,
        has_data: account.data_hash != Hash32::ZERO,
        last_lt: account.last_lt,
    }))
}

fn metadata_status(account: &l2_core::Account) -> &'static str {
    if account.flags.disabled {
        "disabled"
    } else if account.is_recovery_locked() {
        "recovery_locked"
    } else {
        "active"
    }
}
