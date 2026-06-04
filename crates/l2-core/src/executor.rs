use crate::crypto::Hash32;
use crate::gas::{GasFee, GasSchedule};
use crate::state::State;
use crate::types::{
    L2TransactionKind, Receipt, SignedL2Transaction, WithdrawalLeaf, L2_NATIVE_GAS_ASSET,
};
use crate::withdrawal::validate_release_parts;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub block_time: u64,
    pub block_height: u64,
    pub gas_coin_asset: u32,
    pub gas_schedule: GasSchedule,
    pub max_internal_messages: u32,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            block_time: 0,
            block_height: 0,
            gas_coin_asset: L2_NATIVE_GAS_ASSET,
            gas_schedule: GasSchedule::default(),
            max_internal_messages: 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub receipt: Receipt,
    pub withdrawals: Vec<WithdrawalLeaf>,
}

#[derive(Clone, Debug, Default)]
pub struct DeterministicExecutor;

impl DeterministicExecutor {
    pub fn apply(
        &self,
        state: &mut State,
        tx: &SignedL2Transaction,
        config: &ExecutionConfig,
    ) -> ExecutionOutcome {
        let tx_hash = tx.tx_hash();

        match &tx.kind {
            L2TransactionKind::Deposit {
                asset_id,
                recipient,
                amount,
                ..
            } => {
                let account = state.account_mut(*recipient);
                if !account.credit(*asset_id, *amount) {
                    return rejected(tx_hash, "balance_overflow");
                }
                account.last_lt = config.block_height;
                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, 0, None),
                    withdrawals: vec![],
                }
            }
            L2TransactionKind::Transfer {
                to,
                asset_id,
                amount,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                let recipient_can_credit = state
                    .account(*to)
                    .map_or(true, |account| account.can_credit(*asset_id, *amount));
                if !recipient_can_credit {
                    return rejected_attempt(state, tx, from, config, "balance_overflow");
                }
                if let Err(reason) = validate_total_debit(
                    state,
                    from,
                    *asset_id,
                    *amount,
                    config.gas_coin_asset,
                    fee,
                ) {
                    return rejected_attempt(state, tx, from, config, reason);
                }
                if !debit_total(state, from, *asset_id, *amount, config.gas_coin_asset, fee) {
                    return rejected_attempt(state, tx, from, config, "insufficient_balance");
                }

                {
                    let sender = state.account_mut(from);
                    mark_sender_attempt(sender, config.block_height);
                }
                let recipient = state.account_mut(*to);
                if !recipient.credit(*asset_id, *amount) {
                    return rejected_attempt(state, tx, from, config, "balance_overflow");
                }
                recipient.last_lt = config.block_height;

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, None),
                    withdrawals: vec![],
                }
            }
            L2TransactionKind::Withdraw {
                asset_id,
                amount,
                l1_recipient,
            } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                let fee = match execution_fee(tx, config) {
                    Ok(fee) => fee,
                    Err(error) => {
                        return rejected_attempt(state, tx, from, config, error.rejection_reason());
                    }
                };
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                if let Err(error) = validate_release_parts(*amount, l1_recipient) {
                    return rejected_attempt(state, tx, from, config, error.rejection_reason());
                }
                if let Err(reason) = validate_total_debit(
                    state,
                    from,
                    *asset_id,
                    *amount,
                    config.gas_coin_asset,
                    fee,
                ) {
                    return rejected_attempt(state, tx, from, config, reason);
                }
                if !debit_total(state, from, *asset_id, *amount, config.gas_coin_asset, fee) {
                    return rejected_attempt(state, tx, from, config, "insufficient_balance");
                }

                let withdrawal =
                    WithdrawalLeaf::new(tx_hash, *asset_id, *amount, from, l1_recipient.clone());
                {
                    let sender = state.account_mut(from);
                    mark_sender_attempt(sender, config.block_height);
                }

                ExecutionOutcome {
                    receipt: Receipt::applied(tx_hash, fee.amount, Some(withdrawal.withdrawal_id)),
                    withdrawals: vec![withdrawal],
                }
            }
            L2TransactionKind::CallContract { .. } => {
                let from = match authenticated_sender(state, tx) {
                    Ok(from) => from,
                    Err(reason) => return rejected(tx_hash, reason),
                };
                if let Err(error) = execution_fee(tx, config) {
                    return rejected_attempt(state, tx, from, config, error.rejection_reason());
                }
                if !can_increment_nonce(state, from) {
                    return rejected(tx_hash, "nonce_overflow");
                }
                rejected_attempt(state, tx, from, config, "tvm_adapter_not_implemented")
            }
        }
    }
}

fn rejected(tx_hash: Hash32, reason: impl Into<String>) -> ExecutionOutcome {
    ExecutionOutcome {
        receipt: Receipt::rejected(tx_hash, reason),
        withdrawals: vec![],
    }
}

fn rejected_attempt(
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
    }
}

fn execution_fee(
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
) -> Result<GasFee, crate::gas::GasError> {
    config
        .gas_schedule
        .execution_fee(&tx.kind, tx.gas_limit, tx.max_gas_price)
}

fn authenticated_sender(state: &State, tx: &SignedL2Transaction) -> Result<Hash32, &'static str> {
    let from = tx.from.ok_or("missing_sender")?;
    if state.account(from).is_none() {
        return Err("unknown_sender");
    }
    Ok(from)
}

fn can_increment_nonce(state: &State, from: Hash32) -> bool {
    state
        .account(from)
        .is_some_and(|account| account.nonce < u64::MAX)
}

fn mark_sender_attempt(account: &mut crate::state::Account, block_height: u64) {
    account.nonce += 1;
    account.last_lt = block_height;
}

fn charge_rejection_fee(
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
    let gas_charged = if fee > 0 && account.balance(config.gas_coin_asset) >= fee {
        if account.debit(config.gas_coin_asset, fee) {
            fee
        } else {
            0
        }
    } else {
        0
    };
    mark_sender_attempt(account, config.block_height);
    gas_charged
}

fn validate_total_debit(
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

fn debit_total(
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

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
