use super::helpers::{
    optional, optional_secret, optional_string, parse_network, parse_u32_list, required,
};
use super::parser_helpers::{
    bool_value, ensure_asset_list_contains_ton, number_u128, number_u16, number_u32, number_u64,
    number_u8, number_usize, parse_gas_schedule, secret,
};
use super::runtime::parse_runtime_mode;
use super::*;
use anyhow::Context;
use std::path::PathBuf;

impl NodeConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::from_filename(".env.local");
        let _ = dotenvy::dotenv();
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> anyhow::Result<Self> {
        let runtime_mode = parse_runtime_mode(&optional(
            &mut lookup,
            "L2_RUNTIME_MODE",
            DEFAULT_RUNTIME_MODE,
        ))?;
        let l1_ton_asset_id = number_u32(&mut lookup, "L1_TON_ASSET_ID", DEFAULT_L1_TON_ASSET_ID)?;
        let mut l1_deposit_asset_ids = parse_u32_list(
            &optional(
                &mut lookup,
                "L1_DEPOSIT_ASSET_IDS",
                &l1_ton_asset_id.to_string(),
            ),
            "L1_DEPOSIT_ASSET_IDS",
        )?;
        ensure_asset_list_contains_ton(&mut l1_deposit_asset_ids, l1_ton_asset_id);

        let config = Self {
            l2_name: optional(&mut lookup, "L2_NAME", DEFAULT_L2_NAME),
            chain_id: optional(&mut lookup, "L2_CHAIN_ID", DEFAULT_CHAIN_ID),
            native_token_name: optional(&mut lookup, "L2_NATIVE_TOKEN_NAME", DEFAULT_TOKEN_NAME),
            native_token_symbol: optional(
                &mut lookup,
                "L2_NATIVE_TOKEN_SYMBOL",
                DEFAULT_TOKEN_SYMBOL,
            ),
            runtime_mode,
            node_addr: optional(&mut lookup, "L2_NODE_ADDR", DEFAULT_NODE_ADDR)
                .parse()
                .context("invalid L2_NODE_ADDR")?,
            ton_network: parse_network(&required(&mut lookup, "TON_NETWORK")?)?,
            toncenter_v3_base_url: optional(
                &mut lookup,
                "TONCENTER_V3_BASE_URL",
                DEFAULT_TONCENTER_TESTNET,
            ),
            toncenter_api_key: secret(&mut lookup, "TONCENTER_API_KEY")?,
            tonapi_base_url: optional(&mut lookup, "TONAPI_BASE_URL", DEFAULT_TONAPI_TESTNET),
            tonapi_key: secret(&mut lookup, "TONAPI_KEY")?,
            database_url: secret(&mut lookup, "DATABASE_URL")?,
            redis_url: secret(&mut lookup, "REDIS_URL")?,
            admin_token: secret(&mut lookup, "L2_ADMIN_TOKEN")?,
            challenge_window_sec: number_u32(
                &mut lookup,
                "L2_CHALLENGE_WINDOW_SEC",
                DEFAULT_CHALLENGE_WINDOW_SEC,
            )?,
            ent_faucet_amount: number_u128(
                &mut lookup,
                "ENT_FAUCET_AMOUNT",
                DEFAULT_ENT_FAUCET_AMOUNT,
            )?,
            ent_decimals: number_u8(&mut lookup, "ENT_DECIMALS", DEFAULT_ENT_DECIMALS)?,
            ent_logo_path: PathBuf::from(optional(
                &mut lookup,
                "ENT_LOGO_PATH",
                DEFAULT_ENT_LOGO_PATH,
            )),
            ent_faucet_require_admin: bool_value(
                &mut lookup,
                "ENT_FAUCET_REQUIRE_ADMIN",
                DEFAULT_ENT_FAUCET_REQUIRE_ADMIN,
            )?,
            l1_deposit_indexer_enabled: bool_value(
                &mut lookup,
                "L1_DEPOSIT_INDEXER_ENABLED",
                runtime_mode.default_deposit_indexer(),
            )?,
            l1_vault_address: optional_string(&mut lookup, "L1_VAULT_ADDRESS"),
            l1_deposit_poll_interval_ms: number_u64(
                &mut lookup,
                "L1_DEPOSIT_POLL_INTERVAL_MS",
                DEFAULT_L1_DEPOSIT_POLL_INTERVAL_MS,
            )?,
            l1_deposit_batch_limit: number_u16(
                &mut lookup,
                "L1_DEPOSIT_BATCH_LIMIT",
                DEFAULT_L1_DEPOSIT_BATCH_LIMIT,
            )?,
            l1_deposit_confirmation_lag_lt: number_u64(
                &mut lookup,
                "L1_DEPOSIT_CONFIRMATION_LAG_LT",
                DEFAULT_L1_DEPOSIT_CONFIRMATION_LAG_LT,
            )?,
            l1_ton_asset_id,
            l1_deposit_asset_ids,
            dev_admin_deposits_enabled: bool_value(
                &mut lookup,
                "L2_DEV_ADMIN_DEPOSITS_ENABLED",
                runtime_mode.default_dev_admin_deposits(),
            )?,
            l1_batch_relayer_enabled: bool_value(
                &mut lookup,
                "L1_BATCH_RELAYER_ENABLED",
                runtime_mode.default_batch_relayer(),
            )?,
            l1_rollup_root_address: optional_string(&mut lookup, "L1_ROLLUP_ROOT_ADDRESS"),
            l1_sequencer_sender_address: optional_string(
                &mut lookup,
                "L1_SEQUENCER_SENDER_ADDRESS",
            ),
            l1_commit_signer_endpoint: optional_string(&mut lookup, "L1_COMMIT_SIGNER_ENDPOINT"),
            l1_commit_signer_token: optional_secret(&mut lookup, "L1_COMMIT_SIGNER_TOKEN")?,
            l1_commit_msg_value_nanoton: number_u64(
                &mut lookup,
                "L1_COMMIT_MSG_VALUE_NANOTON",
                DEFAULT_L1_COMMIT_MSG_VALUE_NANOTON,
            )?,
            l1_batch_relayer_poll_interval_ms: number_u64(
                &mut lookup,
                "L1_BATCH_RELAYER_POLL_INTERVAL_MS",
                DEFAULT_L1_BATCH_RELAYER_POLL_INTERVAL_MS,
            )?,
            l1_batch_relayer_retry_backoff_ms: number_u64(
                &mut lookup,
                "L1_BATCH_RELAYER_RETRY_BACKOFF_MS",
                DEFAULT_L1_BATCH_RELAYER_RETRY_BACKOFF_MS,
            )?,
            l1_batch_relayer_max_attempts: number_u32(
                &mut lookup,
                "L1_BATCH_RELAYER_MAX_ATTEMPTS",
                DEFAULT_L1_BATCH_RELAYER_MAX_ATTEMPTS,
            )?,
            da_max_payload_bytes: number_usize(
                &mut lookup,
                "DA_MAX_PAYLOAD_BYTES",
                DEFAULT_DA_MAX_PAYLOAD_BYTES,
            )?,
            mempool_replay_ttl_secs: number_u64(
                &mut lookup,
                "MEMPOOL_REPLAY_TTL_SECS",
                DEFAULT_MEMPOOL_REPLAY_TTL_SECS,
            )?,
            mempool_nonce_lock_ttl_secs: number_u64(
                &mut lookup,
                "MEMPOOL_NONCE_LOCK_TTL_SECS",
                DEFAULT_MEMPOOL_NONCE_LOCK_TTL_SECS,
            )?,
            mempool_leader_ttl_secs: number_u64(
                &mut lookup,
                "MEMPOOL_LEADER_TTL_SECS",
                DEFAULT_MEMPOOL_LEADER_TTL_SECS,
            )?,
            mempool_rate_limit_window_secs: number_u64(
                &mut lookup,
                "MEMPOOL_RATE_LIMIT_WINDOW_SECS",
                DEFAULT_MEMPOOL_RATE_LIMIT_WINDOW_SECS,
            )?,
            mempool_max_global_queue: number_usize(
                &mut lookup,
                "MEMPOOL_MAX_GLOBAL_QUEUE",
                DEFAULT_MEMPOOL_MAX_GLOBAL_QUEUE,
            )?,
            mempool_max_account_queue: number_usize(
                &mut lookup,
                "MEMPOOL_MAX_ACCOUNT_QUEUE",
                DEFAULT_MEMPOOL_MAX_ACCOUNT_QUEUE,
            )?,
            mempool_max_account_submissions_per_window: number_u32(
                &mut lookup,
                "MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW",
                DEFAULT_MEMPOOL_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW,
            )?,
            mempool_max_payload_bytes: number_usize(
                &mut lookup,
                "MEMPOOL_MAX_PAYLOAD_BYTES",
                DEFAULT_MEMPOOL_MAX_PAYLOAD_BYTES,
            )?,
            mempool_max_call_body_boc_base64_bytes: number_usize(
                &mut lookup,
                "MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES",
                DEFAULT_MEMPOOL_MAX_CALL_BODY_BOC_BASE64_BYTES,
            )?,
            mempool_min_gas_limit: number_u64(
                &mut lookup,
                "MEMPOOL_MIN_GAS_LIMIT",
                DEFAULT_MEMPOOL_MIN_GAS_LIMIT,
            )?,
            mempool_max_gas_limit: number_u64(
                &mut lookup,
                "MEMPOOL_MAX_GAS_LIMIT",
                DEFAULT_MEMPOOL_MAX_GAS_LIMIT,
            )?,
            mempool_min_gas_price: number_u128(
                &mut lookup,
                "MEMPOOL_MIN_GAS_PRICE",
                DEFAULT_MEMPOOL_MIN_GAS_PRICE,
            )?,
            mempool_max_tx_fee: number_u128(
                &mut lookup,
                "MEMPOOL_MAX_TX_FEE",
                DEFAULT_MEMPOOL_MAX_TX_FEE,
            )?,
            mempool_pop_batch_size: number_usize(
                &mut lookup,
                "MEMPOOL_POP_BATCH_SIZE",
                DEFAULT_MEMPOOL_POP_BATCH_SIZE,
            )?,
            executor_gas_schedule: parse_gas_schedule(&mut lookup)?,
        };
        config.validate()?;
        Ok(config)
    }
}
