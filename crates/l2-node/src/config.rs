use std::net::SocketAddr;
use std::path::PathBuf;

mod debug;
mod defaults;
#[path = "config_helpers.rs"]
mod helpers;
mod parser;
mod parser_helpers;
mod runtime;
mod validation;

pub(crate) use defaults::*;
pub use helpers::SecretString;
pub use runtime::{RuntimeMode, StartupSummary};

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
    pub runtime_mode: RuntimeMode,
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
    pub fn ent_gas_asset_id(&self) -> u32 {
        l2_core::L2_NATIVE_GAS_ASSET
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "config_runtime_tests.rs"]
mod runtime_tests;
