use crate::crypto::Hash32;
use crate::state::State;
use crate::types::L2_NATIVE_GAS_ASSET;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const DEFAULT_MINIMUM_STAKE_ENT: u128 = 1_000_000_000;
pub const DEFAULT_UNBONDING_PERIOD_BLOCKS: u64 = 7_200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StakingConfig {
    pub minimum_stake_ent: u128,
    pub unbonding_period_blocks: u64,
    pub reward_asset_id: u32,
}

impl Default for StakingConfig {
    fn default() -> Self {
        Self {
            minimum_stake_ent: DEFAULT_MINIMUM_STAKE_ENT,
            unbonding_period_blocks: DEFAULT_UNBONDING_PERIOD_BLOCKS,
            reward_asset_id: L2_NATIVE_GAS_ASSET,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidatorStake {
    pub self_bonded: u128,
    pub delegated: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnbondingEntry {
    pub amount: u128,
    pub eligible_block: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StakingState {
    pub validators: BTreeMap<Hash32, ValidatorStake>,
    pub delegations: BTreeMap<Hash32, BTreeMap<Hash32, u128>>,
    pub unbonding: BTreeMap<Hash32, Vec<UnbondingEntry>>,
    pub rewards: BTreeMap<Hash32, u128>,
    pub processed_rewards: BTreeSet<Hash32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewardDistribution {
    pub reward_id: Hash32,
    pub validator: Hash32,
    pub total_amount: u128,
    pub validator_amount: u128,
    pub delegator_amount: u128,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum StakingError {
    #[error("staking config is invalid")]
    InvalidConfig,
    #[error("stake amount is below minimum")]
    BelowMinimumStake,
    #[error("amount must be non-zero")]
    ZeroAmount,
    #[error("account balance is insufficient")]
    InsufficientBalance,
    #[error("validator is unknown")]
    UnknownValidator,
    #[error("delegation is insufficient")]
    InsufficientDelegation,
    #[error("self stake is insufficient")]
    InsufficientSelfStake,
    #[error("validator still has delegations")]
    ValidatorHasDelegations,
    #[error("unbonding entry is not mature")]
    NoEligibleUnbonding,
    #[error("reward id was already processed")]
    DuplicateReward,
    #[error("staking arithmetic overflow")]
    Overflow,
}

impl StakingError {
    pub fn rejection_reason(self) -> &'static str {
        match self {
            Self::InvalidConfig => "staking_invalid_config",
            Self::BelowMinimumStake => "stake_below_minimum",
            Self::ZeroAmount => "staking_zero_amount",
            Self::InsufficientBalance => "staking_insufficient_balance",
            Self::UnknownValidator => "staking_unknown_validator",
            Self::InsufficientDelegation => "staking_insufficient_delegation",
            Self::InsufficientSelfStake => "staking_insufficient_self_stake",
            Self::ValidatorHasDelegations => "staking_validator_has_delegations",
            Self::NoEligibleUnbonding => "staking_no_eligible_unbonding",
            Self::DuplicateReward => "staking_duplicate_reward",
            Self::Overflow => "staking_overflow",
        }
    }
}

impl StakingConfig {
    pub fn validate(&self) -> Result<(), StakingError> {
        if self.minimum_stake_ent == 0 || self.unbonding_period_blocks == 0 {
            return Err(StakingError::InvalidConfig);
        }
        Ok(())
    }
}

impl StakingState {
    pub fn stake(
        &mut self,
        accounts: &mut State,
        config: &StakingConfig,
        staker: Hash32,
        amount: u128,
    ) -> Result<(), StakingError> {
        config.validate()?;
        ensure_amount(amount)?;
        let existing = self.validators.get(&staker).copied().unwrap_or_default();
        let next_self = existing
            .self_bonded
            .checked_add(amount)
            .ok_or(StakingError::Overflow)?;
        if existing.self_bonded == 0 && next_self < config.minimum_stake_ent {
            return Err(StakingError::BelowMinimumStake);
        }
        debit_account(accounts, staker, config.reward_asset_id, amount)?;
        self.validators.insert(
            staker,
            ValidatorStake {
                self_bonded: next_self,
                delegated: existing.delegated,
            },
        );
        Ok(())
    }

    pub fn delegate(
        &mut self,
        accounts: &mut State,
        config: &StakingConfig,
        delegator: Hash32,
        validator: Hash32,
        amount: u128,
    ) -> Result<(), StakingError> {
        config.validate()?;
        ensure_amount(amount)?;
        let current = self
            .validators
            .get(&validator)
            .copied()
            .ok_or(StakingError::UnknownValidator)?;
        if current.self_bonded < config.minimum_stake_ent {
            return Err(StakingError::BelowMinimumStake);
        }
        let next_delegated = current
            .delegated
            .checked_add(amount)
            .ok_or(StakingError::Overflow)?;
        debit_account(accounts, delegator, config.reward_asset_id, amount)?;
        let delegations = self.delegations.entry(validator).or_default();
        let current_delegation = delegations.get(&delegator).copied().unwrap_or_default();
        delegations.insert(
            delegator,
            current_delegation
                .checked_add(amount)
                .ok_or(StakingError::Overflow)?,
        );
        self.validators.insert(
            validator,
            ValidatorStake {
                self_bonded: current.self_bonded,
                delegated: next_delegated,
            },
        );
        Ok(())
    }

    pub fn undelegate(
        &mut self,
        config: &StakingConfig,
        delegator: Hash32,
        validator: Hash32,
        amount: u128,
        current_block: u64,
    ) -> Result<(), StakingError> {
        config.validate()?;
        ensure_amount(amount)?;
        let delegations = self
            .delegations
            .get_mut(&validator)
            .ok_or(StakingError::InsufficientDelegation)?;
        let delegated = delegations
            .get_mut(&delegator)
            .ok_or(StakingError::InsufficientDelegation)?;
        if *delegated < amount {
            return Err(StakingError::InsufficientDelegation);
        }
        *delegated -= amount;
        if *delegated == 0 {
            delegations.remove(&delegator);
        }
        let validator_stake = self
            .validators
            .get_mut(&validator)
            .ok_or(StakingError::UnknownValidator)?;
        validator_stake.delegated = validator_stake
            .delegated
            .checked_sub(amount)
            .ok_or(StakingError::Overflow)?;
        self.push_unbonding(config, delegator, amount, current_block)
    }

    pub fn unbond(
        &mut self,
        config: &StakingConfig,
        validator: Hash32,
        amount: u128,
        current_block: u64,
    ) -> Result<(), StakingError> {
        config.validate()?;
        ensure_amount(amount)?;
        let stake = self
            .validators
            .get_mut(&validator)
            .ok_or(StakingError::UnknownValidator)?;
        if stake.self_bonded < amount {
            return Err(StakingError::InsufficientSelfStake);
        }
        let remaining = stake.self_bonded - amount;
        if remaining > 0 && remaining < config.minimum_stake_ent {
            return Err(StakingError::BelowMinimumStake);
        }
        if remaining == 0 && stake.delegated > 0 {
            return Err(StakingError::ValidatorHasDelegations);
        }
        stake.self_bonded = remaining;
        self.push_unbonding(config, validator, amount, current_block)
    }

    pub fn withdraw_unbonded(
        &mut self,
        accounts: &mut State,
        config: &StakingConfig,
        account_id: Hash32,
        current_block: u64,
    ) -> Result<u128, StakingError> {
        config.validate()?;
        let entries = self
            .unbonding
            .get(&account_id)
            .ok_or(StakingError::NoEligibleUnbonding)?;
        let mut total = 0u128;
        for entry in entries
            .iter()
            .filter(|entry| entry.eligible_block <= current_block)
        {
            total = total
                .checked_add(entry.amount)
                .ok_or(StakingError::Overflow)?;
        }
        if total == 0 {
            return Err(StakingError::NoEligibleUnbonding);
        }
        credit_account(accounts, account_id, config.reward_asset_id, total)?;
        let entries = self.unbonding.get_mut(&account_id).expect("checked above");
        entries.retain(|entry| entry.eligible_block > current_block);
        if entries.is_empty() {
            self.unbonding.remove(&account_id);
        }
        Ok(total)
    }

    pub fn distribute_reward(
        &mut self,
        accounts: &mut State,
        config: &StakingConfig,
        request: RewardRequest,
    ) -> Result<RewardDistribution, StakingError> {
        config.validate()?;
        ensure_amount(request.amount)?;
        if request.commission_bps > 10_000 {
            return Err(StakingError::InvalidConfig);
        }
        if self.processed_rewards.contains(&request.reward_id) {
            return Err(StakingError::DuplicateReward);
        }
        let stake = self
            .validators
            .get(&request.validator)
            .copied()
            .ok_or(StakingError::UnknownValidator)?;
        let total_stake = stake
            .self_bonded
            .checked_add(stake.delegated)
            .ok_or(StakingError::Overflow)?;
        if total_stake == 0 {
            return Err(StakingError::UnknownValidator);
        }
        ensure_balance(
            accounts,
            request.payer,
            config.reward_asset_id,
            request.amount,
        )?;

        let commission = proportional(request.amount, request.commission_bps)?;
        let distributable = request
            .amount
            .checked_sub(commission)
            .ok_or(StakingError::Overflow)?;
        let validator_share = proportional_by_stake(distributable, stake.self_bonded, total_stake)?;
        let mut credits = BTreeMap::<Hash32, u128>::new();
        add_credit(&mut credits, request.validator, commission)?;
        add_credit(&mut credits, request.validator, validator_share)?;
        let mut assigned = validator_share;
        if let Some(delegations) = self.delegations.get(&request.validator) {
            for (delegator, amount) in delegations {
                let share = proportional_by_stake(distributable, *amount, total_stake)?;
                assigned = assigned.checked_add(share).ok_or(StakingError::Overflow)?;
                add_credit(&mut credits, *delegator, share)?;
            }
        }
        let dust = distributable
            .checked_sub(assigned)
            .ok_or(StakingError::Overflow)?;
        add_credit(&mut credits, request.validator, dust)?;
        validate_reward_credits(&self.rewards, &credits)?;

        debit_account(
            accounts,
            request.payer,
            config.reward_asset_id,
            request.amount,
        )?;
        let validator_amount = credits.get(&request.validator).copied().unwrap_or_default();
        for (account, amount) in credits {
            add_reward(&mut self.rewards, account, amount)?;
        }
        self.processed_rewards.insert(request.reward_id);
        Ok(RewardDistribution {
            reward_id: request.reward_id,
            validator: request.validator,
            total_amount: request.amount,
            validator_amount,
            delegator_amount: request
                .amount
                .checked_sub(validator_amount)
                .ok_or(StakingError::Overflow)?,
        })
    }

    pub fn claim_rewards(
        &mut self,
        accounts: &mut State,
        config: &StakingConfig,
        account_id: Hash32,
    ) -> Result<u128, StakingError> {
        config.validate()?;
        let amount = self.rewards.remove(&account_id).unwrap_or_default();
        ensure_amount(amount)?;
        credit_account(accounts, account_id, config.reward_asset_id, amount)?;
        Ok(amount)
    }

    fn push_unbonding(
        &mut self,
        config: &StakingConfig,
        account_id: Hash32,
        amount: u128,
        current_block: u64,
    ) -> Result<(), StakingError> {
        let eligible_block = current_block
            .checked_add(config.unbonding_period_blocks)
            .ok_or(StakingError::Overflow)?;
        self.unbonding
            .entry(account_id)
            .or_default()
            .push(UnbondingEntry {
                amount,
                eligible_block,
            });
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RewardRequest {
    pub reward_id: Hash32,
    pub payer: Hash32,
    pub validator: Hash32,
    pub amount: u128,
    pub commission_bps: u16,
}

fn ensure_amount(amount: u128) -> Result<(), StakingError> {
    if amount == 0 {
        return Err(StakingError::ZeroAmount);
    }
    Ok(())
}

fn ensure_balance(
    state: &State,
    account: Hash32,
    asset_id: u32,
    amount: u128,
) -> Result<(), StakingError> {
    if state
        .account(account)
        .is_some_and(|account| account.balance(asset_id) >= amount)
    {
        return Ok(());
    }
    Err(StakingError::InsufficientBalance)
}

fn debit_account(
    state: &mut State,
    account: Hash32,
    asset_id: u32,
    amount: u128,
) -> Result<(), StakingError> {
    ensure_balance(state, account, asset_id, amount)?;
    state
        .account_mut(account)
        .debit(asset_id, amount)
        .then_some(())
        .ok_or(StakingError::InsufficientBalance)
}

fn credit_account(
    state: &mut State,
    account: Hash32,
    asset_id: u32,
    amount: u128,
) -> Result<(), StakingError> {
    if !state
        .account(account)
        .map_or(true, |account| account.can_credit(asset_id, amount))
    {
        return Err(StakingError::Overflow);
    }
    state
        .account_mut(account)
        .credit(asset_id, amount)
        .then_some(())
        .ok_or(StakingError::Overflow)
}

fn proportional(total: u128, bps: u16) -> Result<u128, StakingError> {
    total
        .checked_mul(u128::from(bps))
        .map(|value| value / 10_000u128)
        .ok_or(StakingError::Overflow)
}

fn proportional_by_stake(total: u128, stake: u128, all_stake: u128) -> Result<u128, StakingError> {
    total
        .checked_mul(stake)
        .map(|value| value / all_stake)
        .ok_or(StakingError::Overflow)
}

fn add_credit(
    credits: &mut BTreeMap<Hash32, u128>,
    account: Hash32,
    amount: u128,
) -> Result<(), StakingError> {
    if amount == 0 {
        return Ok(());
    }
    let current = credits.entry(account).or_default();
    *current = current.checked_add(amount).ok_or(StakingError::Overflow)?;
    Ok(())
}

fn add_reward(
    rewards: &mut BTreeMap<Hash32, u128>,
    account: Hash32,
    amount: u128,
) -> Result<(), StakingError> {
    let current = rewards.entry(account).or_default();
    *current = current.checked_add(amount).ok_or(StakingError::Overflow)?;
    Ok(())
}

fn validate_reward_credits(
    rewards: &BTreeMap<Hash32, u128>,
    credits: &BTreeMap<Hash32, u128>,
) -> Result<(), StakingError> {
    for (account, amount) in credits {
        rewards
            .get(account)
            .copied()
            .unwrap_or_default()
            .checked_add(*amount)
            .ok_or(StakingError::Overflow)?;
    }
    Ok(())
}
