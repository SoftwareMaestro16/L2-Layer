use std::collections::BTreeSet;
use std::time::Duration;

use crate::config::NodeConfig;
use l2_core::Hash32;

const DEFAULT_REPLAY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_NONCE_LOCK_TTL: Duration = Duration::from_secs(5 * 60);
const DEFAULT_LEADER_TTL: Duration = Duration::from_secs(10);
const DEFAULT_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_MAX_GLOBAL_QUEUE: usize = 10_000;
const DEFAULT_MAX_ACCOUNT_QUEUE: usize = 64;
const DEFAULT_MAX_ACCOUNT_NONCE_WINDOW: u64 = 256;
const DEFAULT_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW: u32 = 120;
const DEFAULT_MAX_PAYLOAD_BYTES: usize = 16 * 1024;
const DEFAULT_MAX_TRANSFER_PAYLOAD_BYTES: usize = 4 * 1024;
const DEFAULT_MAX_WITHDRAW_PAYLOAD_BYTES: usize = 4 * 1024;
const DEFAULT_MAX_CALL_PAYLOAD_BYTES: usize = 12 * 1024;
const DEFAULT_MAX_DEPLOY_PAYLOAD_BYTES: usize = 16 * 1024;
const DEFAULT_MAX_CALL_BODY_BOC_BASE64_BYTES: usize = 8 * 1024;
const DEFAULT_MIN_GAS_LIMIT: u64 = 1;
const DEFAULT_MAX_GAS_LIMIT: u64 = 1_000_000;
const DEFAULT_MIN_GAS_PRICE: u128 = 1;
const DEFAULT_MAX_TX_FEE: u128 = 1_000_000_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MempoolAdmissionConfig {
    pub replay_ttl: Duration,
    pub nonce_lock_ttl: Duration,
    pub leader_ttl: Duration,
    pub rate_limit_window: Duration,
    pub max_global_queue: usize,
    pub max_account_queue: usize,
    pub max_account_nonce_window: u64,
    pub max_account_submissions_per_window: u32,
    pub max_payload_bytes: usize,
    pub max_transfer_payload_bytes: usize,
    pub max_withdraw_payload_bytes: usize,
    pub max_call_payload_bytes: usize,
    pub max_deploy_payload_bytes: usize,
    pub max_call_body_boc_base64_bytes: usize,
    pub min_gas_limit: u64,
    pub max_gas_limit: u64,
    pub min_gas_price: u128,
    pub max_tx_fee: u128,
    pub banned_accounts: BTreeSet<Hash32>,
}

impl Default for MempoolAdmissionConfig {
    fn default() -> Self {
        Self {
            replay_ttl: DEFAULT_REPLAY_TTL,
            nonce_lock_ttl: DEFAULT_NONCE_LOCK_TTL,
            leader_ttl: DEFAULT_LEADER_TTL,
            rate_limit_window: DEFAULT_RATE_LIMIT_WINDOW,
            max_global_queue: DEFAULT_MAX_GLOBAL_QUEUE,
            max_account_queue: DEFAULT_MAX_ACCOUNT_QUEUE,
            max_account_nonce_window: DEFAULT_MAX_ACCOUNT_NONCE_WINDOW,
            max_account_submissions_per_window: DEFAULT_MAX_ACCOUNT_SUBMISSIONS_PER_WINDOW,
            max_payload_bytes: DEFAULT_MAX_PAYLOAD_BYTES,
            max_transfer_payload_bytes: DEFAULT_MAX_TRANSFER_PAYLOAD_BYTES,
            max_withdraw_payload_bytes: DEFAULT_MAX_WITHDRAW_PAYLOAD_BYTES,
            max_call_payload_bytes: DEFAULT_MAX_CALL_PAYLOAD_BYTES,
            max_deploy_payload_bytes: DEFAULT_MAX_DEPLOY_PAYLOAD_BYTES,
            max_call_body_boc_base64_bytes: DEFAULT_MAX_CALL_BODY_BOC_BASE64_BYTES,
            min_gas_limit: DEFAULT_MIN_GAS_LIMIT,
            max_gas_limit: DEFAULT_MAX_GAS_LIMIT,
            min_gas_price: DEFAULT_MIN_GAS_PRICE,
            max_tx_fee: DEFAULT_MAX_TX_FEE,
            banned_accounts: BTreeSet::new(),
        }
    }
}

impl MempoolAdmissionConfig {
    pub fn from_config(config: &NodeConfig) -> Self {
        Self {
            replay_ttl: Duration::from_secs(config.mempool_replay_ttl_secs),
            nonce_lock_ttl: Duration::from_secs(config.mempool_nonce_lock_ttl_secs),
            leader_ttl: Duration::from_secs(config.mempool_leader_ttl_secs),
            rate_limit_window: Duration::from_secs(config.mempool_rate_limit_window_secs),
            max_global_queue: config.mempool_max_global_queue,
            max_account_queue: config.mempool_max_account_queue,
            max_account_nonce_window: config.mempool_max_account_nonce_window,
            max_account_submissions_per_window: config.mempool_max_account_submissions_per_window,
            max_payload_bytes: config.mempool_max_payload_bytes,
            max_transfer_payload_bytes: config.mempool_max_transfer_payload_bytes,
            max_withdraw_payload_bytes: config.mempool_max_withdraw_payload_bytes,
            max_call_payload_bytes: config.mempool_max_call_payload_bytes,
            max_deploy_payload_bytes: config.mempool_max_deploy_payload_bytes,
            max_call_body_boc_base64_bytes: config.mempool_max_call_body_boc_base64_bytes,
            min_gas_limit: config.mempool_min_gas_limit,
            max_gas_limit: config.mempool_max_gas_limit,
            min_gas_price: config.mempool_min_gas_price,
            max_tx_fee: config.mempool_max_tx_fee,
            banned_accounts: config.mempool_banned_accounts.iter().copied().collect(),
        }
    }
}
