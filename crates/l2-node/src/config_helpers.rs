use super::TonNetwork;
use anyhow::{anyhow, Context};
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Eq, PartialEq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: String) -> anyhow::Result<Self> {
        let value = value.trim().to_owned();
        if value.is_empty() {
            return Err(anyhow!("secret value must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("\"<redacted>\"")
    }
}

pub(super) fn parse_network(value: &str) -> anyhow::Result<TonNetwork> {
    match value {
        "testnet" => Ok(TonNetwork::Testnet),
        _ => Err(anyhow!("TON_NETWORK must be testnet")),
    }
}

pub(super) fn optional(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
    default: &str,
) -> String {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

pub(super) fn required(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
) -> anyhow::Result<String> {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{key} is required"))
}

pub(super) fn optional_string(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
) -> Option<String> {
    lookup(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(super) fn optional_secret(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
) -> anyhow::Result<Option<SecretString>> {
    lookup(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(SecretString::new)
        .transpose()
}

pub(super) fn parse_u32(value: &str, key: &str) -> anyhow::Result<u32> {
    value
        .parse::<u32>()
        .with_context(|| format!("{key} must be an unsigned 32-bit integer"))
}

pub(super) fn parse_u32_list(value: &str, key: &str) -> anyhow::Result<Vec<u32>> {
    let mut values = vec![];
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(anyhow!("{key} must not contain empty values"));
        }
        values.push(parse_u32(part, key)?);
    }
    values.sort_unstable();
    values.dedup();
    Ok(values)
}

pub(super) fn parse_u16(value: &str, key: &str) -> anyhow::Result<u16> {
    value
        .parse::<u16>()
        .with_context(|| format!("{key} must be an unsigned 16-bit integer"))
}

pub(super) fn parse_u64(value: &str, key: &str) -> anyhow::Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("{key} must be an unsigned 64-bit integer"))
}

pub(super) fn parse_u128(value: &str, key: &str) -> anyhow::Result<u128> {
    value
        .parse::<u128>()
        .with_context(|| format!("{key} must be an unsigned 128-bit integer"))
}

pub(super) fn parse_u8(value: &str, key: &str) -> anyhow::Result<u8> {
    value
        .parse::<u8>()
        .with_context(|| format!("{key} must be an unsigned 8-bit integer"))
}

pub(super) fn parse_bool(value: &str, key: &str) -> anyhow::Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow!("{key} must be true or false")),
    }
}

pub(super) fn bool_literal(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

pub(super) fn path_exists_in_cwd_or_ancestors(path: &PathBuf) -> bool {
    if path.is_absolute() {
        return path.is_file();
    }

    let Ok(mut current) = std::env::current_dir() else {
        return false;
    };
    loop {
        if current.join(path).is_file() {
            return true;
        }
        if !current.pop() {
            return false;
        }
    }
}
