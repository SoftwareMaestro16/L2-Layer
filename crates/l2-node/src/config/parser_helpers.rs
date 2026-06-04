use super::helpers::{
    bool_literal, optional, parse_bool, parse_u128, parse_u16, parse_u32, parse_u64, parse_u8,
    parse_usize, required, SecretString,
};
use super::*;

pub(super) fn secret(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
) -> anyhow::Result<SecretString> {
    SecretString::new(required(lookup, key)?)
}

pub(super) fn bool_value(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: bool,
) -> anyhow::Result<bool> {
    parse_bool(&optional(lookup, key, bool_literal(default)), key)
}

pub(super) fn number_u8(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u8,
) -> anyhow::Result<u8> {
    parse_u8(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn number_u16(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u16,
) -> anyhow::Result<u16> {
    parse_u16(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn number_u32(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u32,
) -> anyhow::Result<u32> {
    parse_u32(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn number_u64(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u64,
) -> anyhow::Result<u64> {
    parse_u64(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn number_u128(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: u128,
) -> anyhow::Result<u128> {
    parse_u128(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn number_usize(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: usize,
) -> anyhow::Result<usize> {
    parse_usize(&optional(lookup, key, &default.to_string()), key)
}

pub(super) fn ensure_asset_list_contains_ton(values: &mut Vec<u32>, ton_asset_id: u32) {
    if !values.contains(&ton_asset_id) {
        values.push(ton_asset_id);
        values.sort_unstable();
        values.dedup();
    }
}

pub(super) fn parse_gas_schedule(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> anyhow::Result<l2_core::GasSchedule> {
    Ok(l2_core::GasSchedule {
        version: number_u32(
            lookup,
            "EXECUTOR_GAS_SCHEDULE_VERSION",
            DEFAULT_EXECUTOR_GAS_SCHEDULE_VERSION,
        )?,
        transfer_gas: number_u64(
            lookup,
            "EXECUTOR_TRANSFER_GAS",
            DEFAULT_EXECUTOR_TRANSFER_GAS,
        )?,
        withdraw_gas: number_u64(
            lookup,
            "EXECUTOR_WITHDRAW_GAS",
            DEFAULT_EXECUTOR_WITHDRAW_GAS,
        )?,
        call_contract_gas: number_u64(
            lookup,
            "EXECUTOR_CALL_CONTRACT_GAS",
            DEFAULT_EXECUTOR_CALL_CONTRACT_GAS,
        )?,
        rejected_execution_gas: number_u64(
            lookup,
            "EXECUTOR_REJECTED_EXECUTION_GAS",
            DEFAULT_EXECUTOR_REJECTED_EXECUTION_GAS,
        )?,
        min_gas_price: number_u128(
            lookup,
            "EXECUTOR_MIN_GAS_PRICE",
            DEFAULT_EXECUTOR_MIN_GAS_PRICE,
        )?,
    })
}
