use super::{ExecutionConfig, ExecutionOutcome};
use crate::address::is_l2_zero_address;
use crate::crypto::{decode_public_key, Hash32};
use crate::gas::GasFee;
use crate::state::{Account, AccountType, State};
use crate::types::{
    Receipt, SignedL2Transaction, L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR,
    L2_TX_VERSION_V2,
};

pub(super) fn rejected(tx_hash: Hash32, reason: impl Into<String>) -> ExecutionOutcome {
    ExecutionOutcome {
        receipt: Receipt::rejected(tx_hash, reason),
        withdrawals: vec![],
        internal_messages: vec![],
    }
}

pub(super) fn validate_execution_envelope(
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

pub(super) fn rejected_attempt(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
    reason: impl Into<String>,
) -> ExecutionOutcome {
    let tx_hash = tx.tx_hash();
    let gas_charged = charge_rejection_fee(state, tx, from, config);
    ExecutionOutcome {
        receipt: Receipt::rejected_with_gas(tx_hash, reason, gas_charged),
        withdrawals: vec![],
        internal_messages: vec![],
    }
}

pub(super) fn execution_fee(
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
) -> Result<GasFee, crate::gas::GasError> {
    config
        .gas_schedule
        .execution_fee(&tx.kind, tx.gas_limit, tx.max_gas_price)
}

pub(super) fn authenticated_sender(
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

pub(super) fn can_increment_nonce(state: &State, from: Hash32) -> bool {
    state
        .account(from)
        .is_some_and(|account| account.nonce < u64::MAX)
}

pub(super) fn mark_sender_attempt(
    account: &mut Account,
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

pub(super) fn charge_rejection_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
) -> u128 {
    if !can_increment_nonce(state, from) {
        return 0;
    }
    let fee = config
        .gas_schedule
        .rejection_fee(tx.gas_limit, tx.max_gas_price)
        .map_or(0, |fee| fee.amount);
    let account = state.account_mut(from);
    let gas_charged = if fee > 0 && account.balance(tx.fee_asset_id) >= fee {
        if account.debit(tx.fee_asset_id, fee) {
            fee
        } else {
            0
        }
    } else {
        0
    };
    mark_sender_attempt(account, tx, config.block_height);
    gas_charged
}

pub(super) fn validate_total_debit(
    state: &State,
    from: Hash32,
    asset_id: u32,
    amount: u128,
    gas_asset_id: u32,
    fee: GasFee,
) -> Result<(), &'static str> {
    let Some(account) = state.account(from) else {
        return Err("unknown_sender");
    };
    if asset_id == gas_asset_id {
        let total = amount.checked_add(fee.amount).ok_or("fee_overflow")?;
        if account.balance(asset_id) < total {
            return Err("insufficient_balance");
        }
    } else {
        if account.balance(gas_asset_id) < fee.amount {
            return Err("insufficient_gas_coin");
        }
        if account.balance(asset_id) < amount {
            return Err("insufficient_balance");
        }
    }
    Ok(())
}

pub(super) fn debit_total(
    state: &mut State,
    from: Hash32,
    asset_id: u32,
    amount: u128,
    gas_asset_id: u32,
    fee: GasFee,
) -> bool {
    let account = state.account_mut(from);
    if asset_id == gas_asset_id {
        let total = match amount.checked_add(fee.amount) {
            Some(total) => total,
            None => return false,
        };
        account.debit(asset_id, total)
    } else if account.balance(gas_asset_id) >= fee.amount && account.balance(asset_id) >= amount {
        account.debit(gas_asset_id, fee.amount) && account.debit(asset_id, amount)
    } else {
        false
    }
}
