use crate::address::is_l2_zero_address;
use crate::crypto::{hash_domain, Hash32};
use crate::state::State;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const BASIS_POINTS_DENOMINATOR: u16 = 10_000;
pub const DEFAULT_OPERATOR_COMMISSION_BPS: u16 = 0;
pub const DEFAULT_TREASURY_FEE_BPS: u16 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeeAccountingConfig {
    pub operator_commission_bps: u16,
    pub treasury_fee_bps: u16,
    pub sequencer_reward_account: Hash32,
    pub operator_fee_account: Hash32,
    pub treasury_fee_account: Hash32,
}

impl Default for FeeAccountingConfig {
    fn default() -> Self {
        Self {
            operator_commission_bps: DEFAULT_OPERATOR_COMMISSION_BPS,
            treasury_fee_bps: DEFAULT_TREASURY_FEE_BPS,
            sequencer_reward_account: default_sequencer_reward_account(),
            operator_fee_account: default_operator_fee_account(),
            treasury_fee_account: default_treasury_fee_account(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeeDistribution {
    pub asset_id: u32,
    pub total_amount: u128,
    pub sequencer_amount: u128,
    pub operator_amount: u128,
    pub treasury_amount: u128,
    pub sequencer_reward_account: Hash32,
    pub operator_fee_account: Hash32,
    pub treasury_fee_account: Hash32,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FeeAccountingError {
    #[error("fee basis points exceed 10000")]
    InvalidBasisPoints,
    #[error("fee distribution overflow")]
    Overflow,
    #[error("fee destination uses reserved zero address")]
    ReservedDestination,
}

impl FeeAccountingError {
    pub fn rejection_reason(self) -> &'static str {
        match self {
            Self::InvalidBasisPoints => "fee_distribution_invalid",
            Self::Overflow => "fee_distribution_overflow",
            Self::ReservedDestination => "fee_destination_reserved",
        }
    }
}

impl FeeAccountingConfig {
    pub fn validate(&self) -> Result<(), FeeAccountingError> {
        if self.operator_commission_bps as u32 + self.treasury_fee_bps as u32
            > BASIS_POINTS_DENOMINATOR as u32
        {
            return Err(FeeAccountingError::InvalidBasisPoints);
        }
        if is_l2_zero_address(self.sequencer_reward_account)
            || is_l2_zero_address(self.operator_fee_account)
            || is_l2_zero_address(self.treasury_fee_account)
        {
            return Err(FeeAccountingError::ReservedDestination);
        }
        Ok(())
    }

    pub fn split_fee(
        &self,
        asset_id: u32,
        total_amount: u128,
    ) -> Result<Option<FeeDistribution>, FeeAccountingError> {
        if total_amount == 0 {
            return Ok(None);
        }
        self.validate()?;
        let operator_amount = proportional_amount(total_amount, self.operator_commission_bps)?;
        let treasury_amount = proportional_amount(total_amount, self.treasury_fee_bps)?;
        let sequencer_amount = total_amount
            .checked_sub(operator_amount)
            .and_then(|value| value.checked_sub(treasury_amount))
            .ok_or(FeeAccountingError::Overflow)?;
        Ok(Some(FeeDistribution {
            asset_id,
            total_amount,
            sequencer_amount,
            operator_amount,
            treasury_amount,
            sequencer_reward_account: self.sequencer_reward_account,
            operator_fee_account: self.operator_fee_account,
            treasury_fee_account: self.treasury_fee_account,
        }))
    }
}

pub fn credit_fee_distribution(
    state: &mut State,
    config: &FeeAccountingConfig,
    asset_id: u32,
    total_amount: u128,
    block_height: u64,
) -> Result<Option<FeeDistribution>, FeeAccountingError> {
    let Some(distribution) = config.split_fee(asset_id, total_amount)? else {
        return Ok(None);
    };
    let mut credits = BTreeMap::<Hash32, u128>::new();
    add_credit(
        &mut credits,
        distribution.sequencer_reward_account,
        distribution.sequencer_amount,
    )?;
    add_credit(
        &mut credits,
        distribution.operator_fee_account,
        distribution.operator_amount,
    )?;
    add_credit(
        &mut credits,
        distribution.treasury_fee_account,
        distribution.treasury_amount,
    )?;
    for (account_id, amount) in &credits {
        if !state
            .account(*account_id)
            .map_or(true, |account| account.can_credit(asset_id, *amount))
        {
            return Err(FeeAccountingError::Overflow);
        }
    }
    for (account_id, amount) in credits {
        let account = state.account_mut(account_id);
        if !account.credit(asset_id, amount) {
            return Err(FeeAccountingError::Overflow);
        }
        account.last_lt = block_height;
    }
    Ok(Some(distribution))
}

pub fn default_sequencer_reward_account() -> Hash32 {
    hash_domain("l2.economics.account.sequencer.v1", &[])
}

pub fn default_operator_fee_account() -> Hash32 {
    hash_domain("l2.economics.account.operator.v1", &[])
}

pub fn default_treasury_fee_account() -> Hash32 {
    hash_domain("l2.economics.account.treasury.v1", &[])
}

fn proportional_amount(total: u128, bps: u16) -> Result<u128, FeeAccountingError> {
    total
        .checked_mul(u128::from(bps))
        .map(|value| value / u128::from(BASIS_POINTS_DENOMINATOR))
        .ok_or(FeeAccountingError::Overflow)
}

fn add_credit(
    credits: &mut BTreeMap<Hash32, u128>,
    account_id: Hash32,
    amount: u128,
) -> Result<(), FeeAccountingError> {
    if amount == 0 {
        return Ok(());
    }
    let current = credits.entry(account_id).or_default();
    *current = current
        .checked_add(amount)
        .ok_or(FeeAccountingError::Overflow)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_fee_rounds_to_sequencer_remainder() {
        let config = FeeAccountingConfig {
            operator_commission_bps: 3333,
            treasury_fee_bps: 3333,
            ..FeeAccountingConfig::default()
        };

        let distribution = config
            .split_fee(0, 100)
            .expect("split")
            .expect("distribution");

        assert_eq!(distribution.operator_amount, 33);
        assert_eq!(distribution.treasury_amount, 33);
        assert_eq!(distribution.sequencer_amount, 34);
    }

    #[test]
    fn invalid_or_overflowing_fee_splits_fail_closed() {
        let invalid = FeeAccountingConfig {
            operator_commission_bps: 9_000,
            treasury_fee_bps: 2_000,
            ..FeeAccountingConfig::default()
        };
        assert_eq!(
            invalid.split_fee(0, 1).unwrap_err(),
            FeeAccountingError::InvalidBasisPoints
        );

        let overflowing = FeeAccountingConfig {
            operator_commission_bps: BASIS_POINTS_DENOMINATOR,
            ..FeeAccountingConfig::default()
        };
        assert_eq!(
            overflowing.split_fee(0, u128::MAX).unwrap_err(),
            FeeAccountingError::Overflow
        );
    }
}
