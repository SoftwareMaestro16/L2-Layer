use super::{can_increment_nonce, mark_sender_attempt, ExecutionConfig};
use crate::economics::{credit_fee_distribution, FeeDistribution};
use crate::gas::GasFee;
use crate::state::State;
use crate::types::{L2Event, SignedL2Transaction};
use crate::Hash32;

pub(in crate::executor) fn charge_rejection_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    from: Hash32,
    config: &ExecutionConfig,
) -> Result<(u128, Option<FeeDistribution>), &'static str> {
    if !can_increment_nonce(state, from) {
        return Ok((0, None));
    }
    let fee = config
        .gas_schedule
        .rejection_fee(tx.gas_limit, tx.max_gas_price)
        .map_or(0, |fee| fee.amount);
    let state_before = state.clone();
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
    let distribution = distribute_charged_fee(state, tx, config, gas_charged, &state_before)?;
    Ok((gas_charged, distribution))
}

pub(in crate::executor) fn validate_total_debit(
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

pub(in crate::executor) fn debit_total(
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

pub(in crate::executor) fn distribute_charged_fee(
    state: &mut State,
    tx: &SignedL2Transaction,
    config: &ExecutionConfig,
    fee_amount: u128,
    state_before: &State,
) -> Result<Option<FeeDistribution>, &'static str> {
    match credit_fee_distribution(
        state,
        &config.fee_accounting,
        tx.fee_asset_id,
        fee_amount,
        config.block_height,
    ) {
        Ok(distribution) => Ok(distribution),
        Err(error) => {
            *state = state_before.clone();
            Err(error.rejection_reason())
        }
    }
}

pub(in crate::executor) fn fee_events(distribution: Option<FeeDistribution>) -> Vec<L2Event> {
    distribution
        .map(|distribution| {
            vec![L2Event::FeeDistributed {
                asset_id: distribution.asset_id,
                total_amount: distribution.total_amount,
                sequencer_amount: distribution.sequencer_amount,
                operator_amount: distribution.operator_amount,
                treasury_amount: distribution.treasury_amount,
                sequencer_reward_account: distribution.sequencer_reward_account,
                operator_fee_account: distribution.operator_fee_account,
                treasury_fee_account: distribution.treasury_fee_account,
            }]
        })
        .unwrap_or_default()
}
