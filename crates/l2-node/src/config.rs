use anyhow::{anyhow, Context};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

const DEFAULT_NODE_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_CHAIN_ID: &str = "entropis-testnet";
const DEFAULT_L2_NAME: &str = "Entropis";
const DEFAULT_TOKEN_NAME: &str = "Entropis";
const DEFAULT_TOKEN_SYMBOL: &str = "ENT";
const DEFAULT_TONCENTER_TESTNET: &str = "https://testnet.toncenter.com/api/v3";
const DEFAULT_TONAPI_TESTNET: &str = "https://testnet.tonapi.io";
const DEFAULT_CHALLENGE_WINDOW_SEC: u32 = 300;
const DEFAULT_ENT_FAUCET_AMOUNT: u128 = 1_000;
const DEFAULT_ENT_DECIMALS: u8 = 9;
const DEFAULT_ENT_LOGO_PATH: &str = "assets/entropis.png";
const DEFAULT_ENT_FAUCET_REQUIRE_ADMIN: bool = true;
const DEFAULT_L1_DEPOSIT_INDEXER_ENABLED: bool = false;
const DEFAULT_L1_DEPOSIT_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_L1_DEPOSIT_BATCH_LIMIT: u16 = 100;
const DEFAULT_L1_DEPOSIT_CONFIRMATION_LAG_LT: u64 = 0;
const DEFAULT_L1_TON_ASSET_ID: u32 = 1;
const DEFAULT_DEV_ADMIN_DEPOSITS_ENABLED: bool = false;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TonNetwork {
    Testnet,
}

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

#[derive(Clone, Eq, PartialEq)]
pub struct NodeConfig {
    pub l2_name: String,
    pub chain_id: String,
    pub native_token_name: String,
    pub native_token_symbol: String,
    pub node_addr: SocketAddr,
    pub ton_network: TonNetwork,
    pub toncenter_v3_base_url: String,
    pub toncenter_api_key: SecretString,
    pub tonapi_base_url: String,
    pub tonapi_key: SecretString,
    pub database_url: SecretString,
    pub redis_url: SecretString,
    pub admin_token: SecretString,
    pub challenge_window_sec: u32,
    pub ent_faucet_amount: u128,
    pub ent_decimals: u8,
    pub ent_logo_path: PathBuf,
    pub ent_faucet_require_admin: bool,
    pub l1_deposit_indexer_enabled: bool,
    pub l1_vault_address: Option<String>,
    pub l1_deposit_poll_interval_ms: u64,
    pub l1_deposit_batch_limit: u16,
    pub l1_deposit_confirmation_lag_lt: u64,
    pub l1_ton_asset_id: u32,
    pub dev_admin_deposits_enabled: bool,
}

impl NodeConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::dotenv();
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> anyhow::Result<Self> {
        let l2_name = optional(&mut lookup, "L2_NAME", DEFAULT_L2_NAME);
        let chain_id = optional(&mut lookup, "L2_CHAIN_ID", DEFAULT_CHAIN_ID);
        let native_token_name = optional(&mut lookup, "L2_NATIVE_TOKEN_NAME", DEFAULT_TOKEN_NAME);
        let native_token_symbol =
            optional(&mut lookup, "L2_NATIVE_TOKEN_SYMBOL", DEFAULT_TOKEN_SYMBOL);
        let node_addr = optional(&mut lookup, "L2_NODE_ADDR", DEFAULT_NODE_ADDR)
            .parse()
            .context("invalid L2_NODE_ADDR")?;
        let ton_network = parse_network(&required(&mut lookup, "TON_NETWORK")?)?;
        let toncenter_v3_base_url = optional(
            &mut lookup,
            "TONCENTER_V3_BASE_URL",
            DEFAULT_TONCENTER_TESTNET,
        );
        let tonapi_base_url = optional(&mut lookup, "TONAPI_BASE_URL", DEFAULT_TONAPI_TESTNET);
        let challenge_window_sec = parse_u32(
            &optional(
                &mut lookup,
                "L2_CHALLENGE_WINDOW_SEC",
                &DEFAULT_CHALLENGE_WINDOW_SEC.to_string(),
            ),
            "L2_CHALLENGE_WINDOW_SEC",
        )?;
        let ent_faucet_amount = parse_u128(
            &optional(
                &mut lookup,
                "ENT_FAUCET_AMOUNT",
                &DEFAULT_ENT_FAUCET_AMOUNT.to_string(),
            ),
            "ENT_FAUCET_AMOUNT",
        )?;
        let ent_decimals = parse_u8(
            &optional(
                &mut lookup,
                "ENT_DECIMALS",
                &DEFAULT_ENT_DECIMALS.to_string(),
            ),
            "ENT_DECIMALS",
        )?;
        let ent_logo_path = PathBuf::from(optional(
            &mut lookup,
            "ENT_LOGO_PATH",
            DEFAULT_ENT_LOGO_PATH,
        ));
        let ent_faucet_require_admin = parse_bool(
            &optional(
                &mut lookup,
                "ENT_FAUCET_REQUIRE_ADMIN",
                bool_literal(DEFAULT_ENT_FAUCET_REQUIRE_ADMIN),
            ),
            "ENT_FAUCET_REQUIRE_ADMIN",
        )?;
        let l1_deposit_indexer_enabled = parse_bool(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_INDEXER_ENABLED",
                bool_literal(DEFAULT_L1_DEPOSIT_INDEXER_ENABLED),
            ),
            "L1_DEPOSIT_INDEXER_ENABLED",
        )?;
        let l1_vault_address = optional(&mut lookup, "L1_VAULT_ADDRESS", "")
            .trim()
            .to_owned();
        let l1_vault_address = (!l1_vault_address.is_empty()).then_some(l1_vault_address);
        let l1_deposit_poll_interval_ms = parse_u64(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_POLL_INTERVAL_MS",
                &DEFAULT_L1_DEPOSIT_POLL_INTERVAL_MS.to_string(),
            ),
            "L1_DEPOSIT_POLL_INTERVAL_MS",
        )?;
        let l1_deposit_batch_limit = parse_u16(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_BATCH_LIMIT",
                &DEFAULT_L1_DEPOSIT_BATCH_LIMIT.to_string(),
            ),
            "L1_DEPOSIT_BATCH_LIMIT",
        )?;
        let l1_deposit_confirmation_lag_lt = parse_u64(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_CONFIRMATION_LAG_LT",
                &DEFAULT_L1_DEPOSIT_CONFIRMATION_LAG_LT.to_string(),
            ),
            "L1_DEPOSIT_CONFIRMATION_LAG_LT",
        )?;
        let l1_ton_asset_id = parse_u32(
            &optional(
                &mut lookup,
                "L1_TON_ASSET_ID",
                &DEFAULT_L1_TON_ASSET_ID.to_string(),
            ),
            "L1_TON_ASSET_ID",
        )?;
        let dev_admin_deposits_enabled = parse_bool(
            &optional(
                &mut lookup,
                "L2_DEV_ADMIN_DEPOSITS_ENABLED",
                bool_literal(DEFAULT_DEV_ADMIN_DEPOSITS_ENABLED),
            ),
            "L2_DEV_ADMIN_DEPOSITS_ENABLED",
        )?;

        let config = Self {
            l2_name,
            chain_id,
            native_token_name,
            native_token_symbol,
            node_addr,
            ton_network,
            toncenter_v3_base_url,
            toncenter_api_key: SecretString::new(required(&mut lookup, "TONCENTER_API_KEY")?)?,
            tonapi_base_url,
            tonapi_key: SecretString::new(required(&mut lookup, "TONAPI_KEY")?)?,
            database_url: SecretString::new(required(&mut lookup, "DATABASE_URL")?)?,
            redis_url: SecretString::new(required(&mut lookup, "REDIS_URL")?)?,
            admin_token: SecretString::new(required(&mut lookup, "L2_ADMIN_TOKEN")?)?,
            challenge_window_sec,
            ent_faucet_amount,
            ent_decimals,
            ent_logo_path,
            ent_faucet_require_admin,
            l1_deposit_indexer_enabled,
            l1_vault_address,
            l1_deposit_poll_interval_ms,
            l1_deposit_batch_limit,
            l1_deposit_confirmation_lag_lt,
            l1_ton_asset_id,
            dev_admin_deposits_enabled,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.ton_network != TonNetwork::Testnet {
            return Err(anyhow!("only TON testnet is allowed for this node"));
        }
        if self.chain_id != DEFAULT_CHAIN_ID {
            return Err(anyhow!("L2_CHAIN_ID must be {DEFAULT_CHAIN_ID}"));
        }
        if !self.toncenter_v3_base_url.contains("testnet.toncenter.com") {
            return Err(anyhow!("TONCENTER_V3_BASE_URL must point to TON testnet"));
        }
        if !self.tonapi_base_url.contains("testnet.tonapi.io") {
            return Err(anyhow!("TONAPI_BASE_URL must point to TON testnet"));
        }
        if !self.database_url.expose().starts_with("postgresql://") {
            return Err(anyhow!("DATABASE_URL must be a PostgreSQL URL"));
        }
        if !self.redis_url.expose().starts_with("redis://") {
            return Err(anyhow!("REDIS_URL must be a Redis URL"));
        }
        if self.admin_token.expose().len() < 16 {
            return Err(anyhow!("L2_ADMIN_TOKEN must be at least 16 bytes"));
        }
        if self.native_token_symbol != DEFAULT_TOKEN_SYMBOL {
            return Err(anyhow!(
                "L2_NATIVE_TOKEN_SYMBOL must be {DEFAULT_TOKEN_SYMBOL}"
            ));
        }
        if self.ent_faucet_amount == 0 {
            return Err(anyhow!("ENT_FAUCET_AMOUNT must be non-zero"));
        }
        if self.ent_decimals != DEFAULT_ENT_DECIMALS {
            return Err(anyhow!("ENT_DECIMALS must be {DEFAULT_ENT_DECIMALS}"));
        }
        if !path_exists_in_cwd_or_ancestors(&self.ent_logo_path) {
            return Err(anyhow!("ENT_LOGO_PATH must point to an existing file"));
        }
        if !self.ent_faucet_require_admin {
            return Err(anyhow!("ENT_FAUCET_REQUIRE_ADMIN must be true for MVP"));
        }
        if self.l1_deposit_indexer_enabled && self.l1_vault_address.is_none() {
            return Err(anyhow!(
                "L1_VAULT_ADDRESS is required when L1_DEPOSIT_INDEXER_ENABLED=true"
            ));
        }
        if self.l1_deposit_poll_interval_ms == 0 {
            return Err(anyhow!("L1_DEPOSIT_POLL_INTERVAL_MS must be non-zero"));
        }
        if self.l1_deposit_batch_limit == 0 || self.l1_deposit_batch_limit > 1000 {
            return Err(anyhow!("L1_DEPOSIT_BATCH_LIMIT must be between 1 and 1000"));
        }
        if self.l1_ton_asset_id == self.ent_gas_asset_id() {
            return Err(anyhow!(
                "L1_TON_ASSET_ID must not equal the ENT gas asset id"
            ));
        }
        Ok(())
    }

    pub fn ent_gas_asset_id(&self) -> u32 {
        l2_core::L2_NATIVE_GAS_ASSET
    }
}

impl fmt::Debug for NodeConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeConfig")
            .field("l2_name", &self.l2_name)
            .field("chain_id", &self.chain_id)
            .field("native_token_name", &self.native_token_name)
            .field("native_token_symbol", &self.native_token_symbol)
            .field("node_addr", &self.node_addr)
            .field("ton_network", &self.ton_network)
            .field("toncenter_v3_base_url", &self.toncenter_v3_base_url)
            .field("toncenter_api_key", &self.toncenter_api_key)
            .field("tonapi_base_url", &self.tonapi_base_url)
            .field("tonapi_key", &self.tonapi_key)
            .field("database_url", &self.database_url)
            .field("redis_url", &self.redis_url)
            .field("admin_token", &self.admin_token)
            .field("challenge_window_sec", &self.challenge_window_sec)
            .field("ent_faucet_amount", &self.ent_faucet_amount)
            .field("ent_decimals", &self.ent_decimals)
            .field("ent_logo_path", &self.ent_logo_path)
            .field("ent_faucet_require_admin", &self.ent_faucet_require_admin)
            .field(
                "l1_deposit_indexer_enabled",
                &self.l1_deposit_indexer_enabled,
            )
            .field("l1_vault_address", &self.l1_vault_address)
            .field(
                "l1_deposit_poll_interval_ms",
                &self.l1_deposit_poll_interval_ms,
            )
            .field("l1_deposit_batch_limit", &self.l1_deposit_batch_limit)
            .field(
                "l1_deposit_confirmation_lag_lt",
                &self.l1_deposit_confirmation_lag_lt,
            )
            .field("l1_ton_asset_id", &self.l1_ton_asset_id)
            .field(
                "dev_admin_deposits_enabled",
                &self.dev_admin_deposits_enabled,
            )
            .finish()
    }
}

fn parse_network(value: &str) -> anyhow::Result<TonNetwork> {
    match value {
        "testnet" => Ok(TonNetwork::Testnet),
        _ => Err(anyhow!("TON_NETWORK must be testnet")),
    }
}

fn optional(lookup: &mut impl FnMut(&str) -> Option<String>, key: &str, default: &str) -> String {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn required(lookup: &mut impl FnMut(&str) -> Option<String>, key: &str) -> anyhow::Result<String> {
    lookup(key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{key} is required"))
}

fn parse_u32(value: &str, key: &str) -> anyhow::Result<u32> {
    value
        .parse::<u32>()
        .with_context(|| format!("{key} must be an unsigned 32-bit integer"))
}

fn parse_u16(value: &str, key: &str) -> anyhow::Result<u16> {
    value
        .parse::<u16>()
        .with_context(|| format!("{key} must be an unsigned 16-bit integer"))
}

fn parse_u64(value: &str, key: &str) -> anyhow::Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("{key} must be an unsigned 64-bit integer"))
}

fn parse_u128(value: &str, key: &str) -> anyhow::Result<u128> {
    value
        .parse::<u128>()
        .with_context(|| format!("{key} must be an unsigned 128-bit integer"))
}

fn parse_u8(value: &str, key: &str) -> anyhow::Result<u8> {
    value
        .parse::<u8>()
        .with_context(|| format!("{key} must be an unsigned 8-bit integer"))
}

fn parse_bool(value: &str, key: &str) -> anyhow::Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow!("{key} must be true or false")),
    }
}

fn bool_literal(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn path_exists_in_cwd_or_ancestors(path: &PathBuf) -> bool {
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

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
