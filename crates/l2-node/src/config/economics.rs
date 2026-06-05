use super::defaults::{DEFAULT_L2_OPERATOR_COMMISSION_BPS, DEFAULT_L2_TREASURY_FEE_BPS};
use super::helpers::{optional, optional_string, parse_u16};
use anyhow::Context;

pub(super) fn parse_fee_accounting(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<l2_core::FeeAccountingConfig> {
    Ok(l2_core::FeeAccountingConfig {
        operator_commission_bps: parse_u16(
            &optional(
                lookup,
                "L2_OPERATOR_COMMISSION_BPS",
                &DEFAULT_L2_OPERATOR_COMMISSION_BPS.to_string(),
            ),
            "L2_OPERATOR_COMMISSION_BPS",
        )?,
        treasury_fee_bps: parse_u16(
            &optional(
                lookup,
                "L2_TREASURY_FEE_BPS",
                &DEFAULT_L2_TREASURY_FEE_BPS.to_string(),
            ),
            "L2_TREASURY_FEE_BPS",
        )?,
        sequencer_reward_account: parse_optional_l2_account(
            lookup,
            "L2_SEQUENCER_REWARD_ACCOUNT",
            l2_core::default_sequencer_reward_account(),
        )?,
        operator_fee_account: parse_optional_l2_account(
            lookup,
            "L2_OPERATOR_FEE_ACCOUNT",
            l2_core::default_operator_fee_account(),
        )?,
        treasury_fee_account: parse_optional_l2_account(
            lookup,
            "L2_TREASURY_FEE_ACCOUNT",
            l2_core::default_treasury_fee_account(),
        )?,
    })
}

fn parse_optional_l2_account(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: l2_core::Hash32,
) -> anyhow::Result<l2_core::Hash32> {
    optional_string(lookup, key)
        .map(|value| {
            l2_core::parse_l2_address(&value)
                .with_context(|| format!("{key} must be an L2 address"))
        })
        .unwrap_or(Ok(default))
}
