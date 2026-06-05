use crate::observer::ObserverReplayConfig;
use l2_core::address::is_l2_zero_address;
use l2_core::crypto::{decode_public_key, derive_account_id, verify_signature};
use l2_core::{
    Account, AccountType, Hash32, L2TransactionKind, SignedL2Transaction,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};

pub(super) fn verify_tx(
    state: &l2_core::State,
    tx: &SignedL2Transaction,
    config: &ObserverReplayConfig,
    block_height: u64,
) -> Result<(), &'static str> {
    if tx.chain_id != config.chain_id {
        return Err("wrong_chain_id");
    }
    validate_tx_envelope(tx, block_height)?;
    if is_canonical_system_tx(tx) {
        return validate_reserved_zero_addresses(tx, true);
    }
    if tx.is_system() {
        return Err(system_tx_rejection_reason(tx));
    }
    if tx.fee_asset_id != config.gas_coin_asset {
        return Err("unsupported_fee_asset");
    }
    let from = tx.from.ok_or("missing_sender")?;
    if is_l2_zero_address(from) {
        return Err("reserved_zero_address");
    }
    let public_key_hex = tx.public_key.as_deref().ok_or("missing_public_key")?;
    let signature_hex = tx.signature.as_deref().ok_or("missing_signature")?;
    let account = state.account(from).ok_or("unknown_sender")?;
    validate_public_sender_account(account)?;
    let public_key = decode_public_key(public_key_hex).map_err(|_| "invalid_public_key")?;
    validate_account_public_key(from, account, &public_key)?;
    if !verify_signature(public_key_hex, signature_hex, &tx.signing_payload()) {
        return Err("bad_signature");
    }
    if account.nonce != tx.nonce {
        return Err("bad_nonce");
    }
    validate_reserved_zero_addresses(tx, false)
}

fn validate_tx_envelope(tx: &SignedL2Transaction, block_height: u64) -> Result<(), &'static str> {
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

fn validate_public_sender_account(account: &Account) -> Result<(), &'static str> {
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

fn validate_account_public_key(
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

fn is_canonical_system_tx(tx: &SignedL2Transaction) -> bool {
    matches!(
        tx.kind,
        L2TransactionKind::Deposit { .. } | L2TransactionKind::InternalMessage { .. }
    ) && tx.from.is_none()
        && tx.public_key.is_none()
        && tx.signature.is_none()
}

fn system_tx_rejection_reason(tx: &SignedL2Transaction) -> &'static str {
    match tx.kind {
        L2TransactionKind::InternalMessage { .. } => "internal_message_must_be_system",
        _ => "deposit_must_be_system",
    }
}

fn validate_reserved_zero_addresses(
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
