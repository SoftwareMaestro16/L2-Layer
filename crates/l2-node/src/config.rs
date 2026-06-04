use anyhow::{anyhow, Context};
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

#[path = "config_helpers.rs"]
mod helpers;

pub use helpers::SecretString;
use helpers::{
    bool_literal, optional, optional_secret, optional_string, parse_bool, parse_network,
    parse_u128, parse_u16, parse_u32, parse_u32_list, parse_u64, parse_u8,
    path_exists_in_cwd_or_ancestors, required,
};

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
const DEFAULT_L1_BATCH_RELAYER_ENABLED: bool = false;
const DEFAULT_L1_COMMIT_MSG_VALUE_NANOTON: u64 = 100_000_000;
const DEFAULT_L1_BATCH_RELAYER_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_L1_BATCH_RELAYER_RETRY_BACKOFF_MS: u64 = 15_000;
const DEFAULT_L1_BATCH_RELAYER_MAX_ATTEMPTS: u32 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TonNetwork {
    Testnet,
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
    pub l1_deposit_asset_ids: Vec<u32>,
    pub dev_admin_deposits_enabled: bool,
    pub l1_batch_relayer_enabled: bool,
    pub l1_rollup_root_address: Option<String>,
    pub l1_sequencer_sender_address: Option<String>,
    pub l1_commit_signer_endpoint: Option<String>,
    pub l1_commit_signer_token: Option<SecretString>,
    pub l1_commit_msg_value_nanoton: u64,
    pub l1_batch_relayer_poll_interval_ms: u64,
    pub l1_batch_relayer_retry_backoff_ms: u64,
    pub l1_batch_relayer_max_attempts: u32,
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
        let mut l1_deposit_asset_ids = parse_u32_list(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_ASSET_IDS",
                &l1_ton_asset_id.to_string(),
            ),
            "L1_DEPOSIT_ASSET_IDS",
        )?;
        if !l1_deposit_asset_ids.contains(&l1_ton_asset_id) {
            l1_deposit_asset_ids.push(l1_ton_asset_id);
            l1_deposit_asset_ids.sort_unstable();
            l1_deposit_asset_ids.dedup();
        }
        let dev_admin_deposits_enabled = parse_bool(
            &optional(
                &mut lookup,
                "L2_DEV_ADMIN_DEPOSITS_ENABLED",
                bool_literal(DEFAULT_DEV_ADMIN_DEPOSITS_ENABLED),
            ),
            "L2_DEV_ADMIN_DEPOSITS_ENABLED",
        )?;
        let l1_batch_relayer_enabled = parse_bool(
            &optional(
                &mut lookup,
                "L1_BATCH_RELAYER_ENABLED",
                bool_literal(DEFAULT_L1_BATCH_RELAYER_ENABLED),
            ),
            "L1_BATCH_RELAYER_ENABLED",
        )?;
        let l1_rollup_root_address = optional_string(&mut lookup, "L1_ROLLUP_ROOT_ADDRESS");
        let l1_sequencer_sender_address =
            optional_string(&mut lookup, "L1_SEQUENCER_SENDER_ADDRESS");
        let l1_commit_signer_endpoint = optional_string(&mut lookup, "L1_COMMIT_SIGNER_ENDPOINT");
        let l1_commit_signer_token = optional_secret(&mut lookup, "L1_COMMIT_SIGNER_TOKEN")?;
        let l1_commit_msg_value_nanoton = parse_u64(
            &optional(
                &mut lookup,
                "L1_COMMIT_MSG_VALUE_NANOTON",
                &DEFAULT_L1_COMMIT_MSG_VALUE_NANOTON.to_string(),
            ),
            "L1_COMMIT_MSG_VALUE_NANOTON",
        )?;
        let l1_batch_relayer_poll_interval_ms = parse_u64(
            &optional(
                &mut lookup,
                "L1_BATCH_RELAYER_POLL_INTERVAL_MS",
                &DEFAULT_L1_BATCH_RELAYER_POLL_INTERVAL_MS.to_string(),
            ),
            "L1_BATCH_RELAYER_POLL_INTERVAL_MS",
        )?;
        let l1_batch_relayer_retry_backoff_ms = parse_u64(
            &optional(
                &mut lookup,
                "L1_BATCH_RELAYER_RETRY_BACKOFF_MS",
                &DEFAULT_L1_BATCH_RELAYER_RETRY_BACKOFF_MS.to_string(),
            ),
            "L1_BATCH_RELAYER_RETRY_BACKOFF_MS",
        )?;
        let l1_batch_relayer_max_attempts = parse_u32(
            &optional(
                &mut lookup,
                "L1_BATCH_RELAYER_MAX_ATTEMPTS",
                &DEFAULT_L1_BATCH_RELAYER_MAX_ATTEMPTS.to_string(),
            ),
            "L1_BATCH_RELAYER_MAX_ATTEMPTS",
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
            l1_deposit_asset_ids,
            dev_admin_deposits_enabled,
            l1_batch_relayer_enabled,
            l1_rollup_root_address,
            l1_sequencer_sender_address,
            l1_commit_signer_endpoint,
            l1_commit_signer_token,
            l1_commit_msg_value_nanoton,
            l1_batch_relayer_poll_interval_ms,
            l1_batch_relayer_retry_backoff_ms,
            l1_batch_relayer_max_attempts,
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
        if self
            .l1_deposit_asset_ids
            .iter()
            .any(|asset_id| *asset_id == self.ent_gas_asset_id())
        {
            return Err(anyhow!(
                "L1_DEPOSIT_ASSET_IDS must not include the ENT gas asset id"
            ));
        }
        if self.l1_batch_relayer_enabled {
            if self.l1_rollup_root_address.is_none() {
                return Err(anyhow!(
                    "L1_ROLLUP_ROOT_ADDRESS is required when L1_BATCH_RELAYER_ENABLED=true"
                ));
            }
            if self.l1_sequencer_sender_address.is_none() {
                return Err(anyhow!(
                    "L1_SEQUENCER_SENDER_ADDRESS is required when L1_BATCH_RELAYER_ENABLED=true"
                ));
            }
            if self.l1_commit_signer_endpoint.is_none() {
                return Err(anyhow!(
                    "L1_COMMIT_SIGNER_ENDPOINT is required when L1_BATCH_RELAYER_ENABLED=true"
                ));
            }
            if self.l1_commit_signer_token.is_none() {
                return Err(anyhow!(
                    "L1_COMMIT_SIGNER_TOKEN is required when L1_BATCH_RELAYER_ENABLED=true"
                ));
            }
        }
        if let Some(endpoint) = self.l1_commit_signer_endpoint.as_deref() {
            if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                return Err(anyhow!("L1_COMMIT_SIGNER_ENDPOINT must be an HTTP URL"));
            }
        }
        if self.l1_commit_msg_value_nanoton == 0 {
            return Err(anyhow!("L1_COMMIT_MSG_VALUE_NANOTON must be non-zero"));
        }
        if self.l1_batch_relayer_poll_interval_ms == 0 {
            return Err(anyhow!(
                "L1_BATCH_RELAYER_POLL_INTERVAL_MS must be non-zero"
            ));
        }
        if self.l1_batch_relayer_retry_backoff_ms == 0 {
            return Err(anyhow!(
                "L1_BATCH_RELAYER_RETRY_BACKOFF_MS must be non-zero"
            ));
        }
        if self.l1_batch_relayer_max_attempts == 0 || self.l1_batch_relayer_max_attempts > 100 {
            return Err(anyhow!(
                "L1_BATCH_RELAYER_MAX_ATTEMPTS must be between 1 and 100"
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
            .field("l1_deposit_asset_ids", &self.l1_deposit_asset_ids)
            .field(
                "dev_admin_deposits_enabled",
                &self.dev_admin_deposits_enabled,
            )
            .field("l1_batch_relayer_enabled", &self.l1_batch_relayer_enabled)
            .field("l1_rollup_root_address", &self.l1_rollup_root_address)
            .field(
                "l1_sequencer_sender_address",
                &self.l1_sequencer_sender_address,
            )
            .field("l1_commit_signer_endpoint", &self.l1_commit_signer_endpoint)
            .field("l1_commit_signer_token", &self.l1_commit_signer_token)
            .field(
                "l1_commit_msg_value_nanoton",
                &self.l1_commit_msg_value_nanoton,
            )
            .field(
                "l1_batch_relayer_poll_interval_ms",
                &self.l1_batch_relayer_poll_interval_ms,
            )
            .field(
                "l1_batch_relayer_retry_backoff_ms",
                &self.l1_batch_relayer_retry_backoff_ms,
            )
            .field(
                "l1_batch_relayer_max_attempts",
                &self.l1_batch_relayer_max_attempts,
            )
            .finish()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
