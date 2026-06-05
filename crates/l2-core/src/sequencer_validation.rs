use crate::address::is_l2_zero_address;
use crate::crypto::{derive_account_id, Hash32};
use crate::state::{Account, AccountType};
use crate::types::{
    L2TransactionKind, SignedL2Transaction, L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR,
    L2_TX_VERSION_V2,
};

pub(super) fn validate_tx_envelope(
    tx: &SignedL2Transaction,
    block_height: u64,
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
    if tx.valid_until_block < block_height {
        return Err("tx_expired");
    }
    Ok(())
}

pub(super) fn validate_public_sender_account(account: &Account) -> Result<(), &'static str> {
    if account.flags.disabled {
        return Err("account_disabled");
    }
    if account.is_recovery_locked() {
        return Err("account_recovery_locked");
    }
    if account.flags.system_only || matches!(account.account_type, AccountType::System) {
        return Err("sender_system_only");
    }
    if account.flags.contract_only || matches!(account.account_type, AccountType::Contract) {
        return Err("sender_contract_only");
    }
    if !account.can_send_public_transaction() {
        return Err("sender_not_public");
    }
    Ok(())
}

pub(super) fn validate_account_public_key(
    from: Hash32,
    account: &Account,
    public_key: &[u8; 32],
) -> Result<(), &'static str> {
    if let Some(active_public_key) = account.active_public_key {
        if active_public_key.as_bytes() == public_key {
            return Ok(());
        }
        return Err("public_key_sender_mismatch");
    }
    if derive_account_id(public_key) != from {
        return Err("public_key_sender_mismatch");
    }
    Ok(())
}

pub(super) fn validate_reserved_zero_addresses(
    tx: &SignedL2Transaction,
    allow_system_deposit: bool,
) -> Result<(), &'static str> {
    match tx.kind {
        L2TransactionKind::Deposit { recipient, .. } if is_l2_zero_address(recipient) => {
            Err("reserved_zero_address")
        }
        L2TransactionKind::Deposit { .. } if allow_system_deposit => Ok(()),
        L2TransactionKind::Deposit { .. } => Err("deposit_must_be_system"),
        L2TransactionKind::Transfer { to, .. } if is_l2_zero_address(to) => {
            Err("reserved_zero_address")
        }
        L2TransactionKind::DeployContract { contract, .. }
        | L2TransactionKind::CallContract { contract, .. }
            if is_l2_zero_address(contract) =>
        {
            Err("reserved_zero_address")
        }
        L2TransactionKind::InternalMessage { from, to, .. }
            if is_l2_zero_address(from) || is_l2_zero_address(to) =>
        {
            Err("reserved_zero_address")
        }
        L2TransactionKind::InternalMessage { .. } if allow_system_deposit => Ok(()),
        L2TransactionKind::InternalMessage { .. } => Err("internal_message_must_be_system"),
        _ => Ok(()),
    }
}
