use anyhow::Context;
use std::net::SocketAddr;
use std::path::PathBuf;

mod debug;
mod defaults;
#[path = "config_helpers.rs"]
mod helpers;
mod validation;

pub(crate) use defaults::*;
pub use helpers::SecretString;
use helpers::{
    bool_literal, optional, optional_secret, optional_string, parse_bool, parse_network,
    parse_u128, parse_u16, parse_u32, parse_u32_list, parse_u64, parse_u8, parse_usize,
    path_exists_in_cwd_or_ancestors, required,
};

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
    pub l1_batch_finalizer_enabled: bool,
    pub l1_finalize_signer_endpoint: Option<String>,
    pub l1_finalize_signer_token: Option<SecretString>,
    pub l1_finalize_msg_value_nanoton: u64,
    pub l1_batch_finalizer_poll_interval_ms: u64,
    pub l1_batch_finalizer_retry_backoff_ms: u64,
    pub l1_batch_finalizer_max_attempts: u32,
    pub da_max_payload_bytes: usize,
    pub mempool_replay_ttl_secs: u64,
    pub mempool_nonce_lock_ttl_secs: u64,
    pub mempool_leader_ttl_secs: u64,
    pub mempool_rate_limit_window_secs: u64,
    pub mempool_max_global_queue: usize,
    pub mempool_max_account_queue: usize,
    pub mempool_max_account_submissions_per_window: u32,
    pub mempool_max_payload_bytes: usize,
    pub mempool_max_call_body_boc_base64_bytes: usize,
    pub mempool_min_gas_limit: u64,
    pub mempool_max_gas_limit: u64,
    pub mempool_min_gas_price: u128,
    pub mempool_max_tx_fee: u128,
    pub mempool_pop_batch_size: usize,
    pub executor_gas_schedule: l2_core::GasSchedule,
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
        let l1_batch_finalizer_enabled = parse_bool(
            &optional(
                &mut lookup,
                "L1_BATCH_FINALIZER_ENABLED",
                bool_literal(DEFAULT_L1_BATCH_FINALIZER_ENABLED),
            ),
            "L1_BATCH_FINALIZER_ENABLED",
        )?;
        let l1_finalize_signer_endpoint =
            optional_string(&mut lookup, "L1_FINALIZE_SIGNER_ENDPOINT");
        let l1_finalize_signer_token = optional_secret(&mut lookup, "L1_FINALIZE_SIGNER_TOKEN")?;
        let l1_finalize_msg_value_nanoton = parse_u64(
            &optional(
                &mut lookup,
                "L1_FINALIZE_MSG_VALUE_NANOTON",
                &DEFAULT_L1_FINALIZE_MSG_VALUE_NANOTON.to_string(),
            ),
            "L1_FINALIZE_MSG_VALUE_NANOTON",
        )?;
        let l1_batch_finalizer_poll_interval_ms = parse_u64(
            &optional(
                &mut lookup,
                "L1_BATCH_FINALIZER_POLL_INTERVAL_MS",
                &DEFAULT_L1_BATCH_FINALIZER_POLL_INTERVAL_MS.to_string(),
            ),
            "L1_BATCH_FINALIZER_POLL_INTERVAL_MS",
        )?;
        let l1_batch_finalizer_retry_backoff_ms = parse_u64(
            &optional(
                &mut lookup,
                "L1_BATCH_FINALIZER_RETRY_BACKOFF_MS",
                &DEFAULT_L1_BATCH_FINALIZER_RETRY_BACKOFF_MS.to_string(),
            ),
            "L1_BATCH_FINALIZER_RETRY_BACKOFF_MS",
        )?;
        let l1_batch_finalizer_max_attempts = parse_u32(
            &optional(
                &mut lookup,
                "L1_BATCH_FINALIZER_MAX_ATTEMPTS",
                &DEFAULT_L1_BATCH_FINALIZER_MAX_ATTEMPTS.to_string(),
            ),
            "L1_BATCH_FINALIZER_MAX_ATTEMPTS",
        )?;
        let da_max_payload_bytes = parse_usize(
            &optional(
                &mut lookup,
                "DA_MAX_PAYLOAD_BYTES",
                &DEFAULT_DA_MAX_PAYLOAD_BYTES.to_string(),
            ),
            "DA_MAX_PAYLOAD_BYTES",
        )?;
        let mempool_replay_ttl_secs = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_REPLAY_TTL_SECS",
                &DEFAULT_MEMPOOL_REPLAY_TTL_SECS.to_string(),
            ),
            "MEMPOOL_REPLAY_TTL_SECS",
        )?;
        let mempool_nonce_lock_ttl_secs = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_NONCE_LOCK_TTL_SECS",
                &DEFAULT_MEMPOOL_NONCE_LOCK_TTL_SECS.to_string(),
            ),
            "MEMPOOL_NONCE_LOCK_TTL_SECS",
        )?;
        let mempool_leader_ttl_secs = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_LEADER_TTL_SECS",
                &DEFAULT_MEMPOOL_LEADER_TTL_SECS.to_string(),
            ),
            "MEMPOOL_LEADER_TTL_SECS",
        )?;
        let mempool_rate_limit_window_secs = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_RATE_LIMIT_WINDOW_SECS",
                &DEFAULT_MEMPOOL_RATE_LIMIT_WINDOW_SECS.to_string(),
            ),
            "MEMPOOL_RATE_LIMIT_WINDOW_SECS",
        )?;
        let mempool_max_global_queue = parse_usize(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_GLOBAL_QUEUE",
                &DEFAULT_MEMPOOL_MAX_GLOBAL_QUEUE.to_string(),
            ),
            "MEMPOOL_MAX_GLOBAL_QUEUE",
        )?;
        let mempool_max_account_queue = parse_usize(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_ACCOUNT_QUEUE",
                &DEFAULT_MEMPOOL_MAX_ACCOUNT_QUEUE.to_string(),
            ),
            "MEMPOOL_MAX_ACCOUNT_QUEUE",
        )?;
        let mempool_max_account_submissions_per_window = parse_u32(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW",
                &DEFAULT_MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW.to_string(),
            ),
            "MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW",
        )?;
        let mempool_max_payload_bytes = parse_usize(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_PAYLOAD_BYTES",
                &DEFAULT_MEMPOOL_MAX_PAYLOAD_BYTES.to_string(),
            ),
            "MEMPOOL_MAX_PAYLOAD_BYTES",
        )?;
        let mempool_max_call_body_boc_base64_bytes = parse_usize(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES",
                &DEFAULT_MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES.to_string(),
            ),
            "MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES",
        )?;
        let mempool_min_gas_limit = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_MIN_GAS_LIMIT",
                &DEFAULT_MEMPOOL_MIN_GAS_LIMIT.to_string(),
            ),
            "MEMPOOL_MIN_GAS_LIMIT",
        )?;
        let mempool_max_gas_limit = parse_u64(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_GAS_LIMIT",
                &DEFAULT_MEMPOOL_MAX_GAS_LIMIT.to_string(),
            ),
            "MEMPOOL_MAX_GAS_LIMIT",
        )?;
        let mempool_min_gas_price = parse_u128(
            &optional(
                &mut lookup,
                "MEMPOOL_MIN_GAS_PRICE",
                &DEFAULT_MEMPOOL_MIN_GAS_PRICE.to_string(),
            ),
            "MEMPOOL_MIN_GAS_PRICE",
        )?;
        let mempool_max_tx_fee = parse_u128(
            &optional(
                &mut lookup,
                "MEMPOOL_MAX_TX_FEE",
                &DEFAULT_MEMPOOL_MAX_TX_FEE.to_string(),
            ),
            "MEMPOOL_MAX_TX_FEE",
        )?;
        let mempool_pop_batch_size = parse_usize(
            &optional(
                &mut lookup,
                "MEMPOOL_POP_BATCH_SIZE",
                &DEFAULT_MEMPOOL_POP_BATCH_SIZE.to_string(),
            ),
            "MEMPOOL_POP_BATCH_SIZE",
        )?;
        let executor_gas_schedule = l2_core::GasSchedule {
            version: parse_u32(
                &optional(
                    &mut lookup,
                    "EXECUTOR_GAS_SCHEDULE_VERSION",
                    &DEFAULT_EXECUTOR_GAS_SCHEDULE_VERSION.to_string(),
                ),
                "EXECUTOR_GAS_SCHEDULE_VERSION",
            )?,
            transfer_gas: parse_u64(
                &optional(
                    &mut lookup,
                    "EXECUTOR_TRANSFER_GAS",
                    &DEFAULT_EXECUTOR_TRANSFER_GAS.to_string(),
                ),
                "EXECUTOR_TRANSFER_GAS",
            )?,
            withdraw_gas: parse_u64(
                &optional(
                    &mut lookup,
                    "EXECUTOR_WITHDRAW_GAS",
                    &DEFAULT_EXECUTOR_WITHDRAW_GAS.to_string(),
                ),
                "EXECUTOR_WITHDRAW_GAS",
            )?,
            call_contract_gas: parse_u64(
                &optional(
                    &mut lookup,
                    "EXECUTOR_CALL_CONTRACT_GAS",
                    &DEFAULT_EXECUTOR_CALL_CONTRACT_GAS.to_string(),
                ),
                "EXECUTOR_CALL_CONTRACT_GAS",
            )?,
            rejected_execution_gas: parse_u64(
                &optional(
                    &mut lookup,
                    "EXECUTOR_REJECTED_EXECUTION_GAS",
                    &DEFAULT_EXECUTOR_REJECTED_EXECUTION_GAS.to_string(),
                ),
                "EXECUTOR_REJECTED_EXECUTION_GAS",
            )?,
            min_gas_price: parse_u128(
                &optional(
                    &mut lookup,
                    "EXECUTOR_MIN_GAS_PRICE",
                    &DEFAULT_EXECUTOR_MIN_GAS_PRICE.to_string(),
                ),
                "EXECUTOR_MIN_GAS_PRICE",
            )?,
        };

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
            l1_batch_finalizer_enabled,
            l1_finalize_signer_endpoint,
            l1_finalize_signer_token,
            l1_finalize_msg_value_nanoton,
            l1_batch_finalizer_poll_interval_ms,
            l1_batch_finalizer_retry_backoff_ms,
            l1_batch_finalizer_max_attempts,
            da_max_payload_bytes,
            mempool_replay_ttl_secs,
            mempool_nonce_lock_ttl_secs,
            mempool_leader_ttl_secs,
            mempool_rate_limit_window_secs,
            mempool_max_global_queue,
            mempool_max_account_queue,
            mempool_max_account_submissions_per_window,
            mempool_max_payload_bytes,
            mempool_max_call_body_boc_base64_bytes,
            mempool_min_gas_limit,
            mempool_max_gas_limit,
            mempool_min_gas_price,
            mempool_max_tx_fee,
            mempool_pop_batch_size,
            executor_gas_schedule,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn ent_gas_asset_id(&self) -> u32 {
        l2_core::L2_NATIVE_GAS_ASSET
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
