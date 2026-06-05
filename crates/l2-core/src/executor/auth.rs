use super::ExecutionConfig;
use crate::address::is_l2_zero_address;
use crate::crypto::{decode_public_key, Hash32};
use crate::state::{Account, AccountType, State};
use crate::types::{
    SignedL2Transaction, L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};

pub(in crate::executor) fn validate_execution_envelope(
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
) -> Result<(), &'static str> {
    if tx.tx_version != L2_TX_VERSION_V2 {
        return Err("unsupported_tx_version");
    }
    if tx.domain_separator != L2_TX_DOMAIN_SEPARATOR {
        return Err("invalid_domain_separator");
    }
    if tx.transaction_kind_version != L2_TRANSACTION_KIND_VERSION_V1 {
        return Err("unsupported_transaction_kind_version");
    }
    if tx.valid_until_block < config.block_height {
        return Err("tx_expired");
    }
    if !tx.is_system() && tx.fee_asset_id != config.gas_coin_asset {
        return Err("unsupported_fee_asset");
    }
    Ok(())
}

pub(in crate::executor) fn authenticated_sender(
    state: &State,
    tx: &SignedL2Transaction,
) -> Result<Hash32, &'static str> {
    let from = tx.from.ok_or("missing_sender")?;
    if is_l2_zero_address(from) {
        return Err("reserved_zero_address");
    }
    let account = state.account(from).ok_or("unknown_sender")?;
    if let Some(reason) = public_sender_rejection(account) {
        return Err(reason);
    }
    Ok(from)
}

pub(in crate::executor) fn can_increment_nonce(state: &State, from: Hash32) -> bool {
    state
        .account(from)
        .is_some_and(|account| account.nonce < u64::MAX)
}

pub(in crate::executor) fn mark_sender_attempt(
    account: &mut crate::state::Account,
    tx: &SignedL2Transaction,
    block_height: u64,
) {
    if account.active_public_key.is_none() {
        if let Some(public_key) = tx
            .public_key
            .as_deref()
            .and_then(|public_key| decode_public_key(public_key).ok())
        {
            account.active_public_key = Some(Hash32::new(public_key));
        }
    }
    account.nonce += 1;
    account.last_lt = block_height;
}

fn public_sender_rejection(account: &Account) -> Option<&'static str> {
    if account.flags.disabled {
        return Some("account_disabled");
    }
    if account.is_recovery_locked() {
        return Some("account_recovery_locked");
    }
    if account.flags.system_only || matches!(account.account_type, AccountType::System) {
        return Some("sender_system_only");
    }
    if account.flags.contract_only || matches!(account.account_type, AccountType::Contract) {
        return Some("sender_contract_only");
    }
    if !account.can_send_public_transaction() {
        return Some("sender_not_public");
    }
    None
}
