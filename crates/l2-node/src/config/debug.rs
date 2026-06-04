use super::NodeConfig;
use std::fmt;

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
            .field(
                "l1_batch_finalizer_enabled",
                &self.l1_batch_finalizer_enabled,
            )
            .field(
                "l1_finalize_signer_endpoint",
                &self.l1_finalize_signer_endpoint,
            )
            .field("l1_finalize_signer_token", &self.l1_finalize_signer_token)
            .field(
                "l1_finalize_msg_value_nanoton",
                &self.l1_finalize_msg_value_nanoton,
            )
            .field(
                "l1_batch_finalizer_poll_interval_ms",
                &self.l1_batch_finalizer_poll_interval_ms,
            )
            .field(
                "l1_batch_finalizer_retry_backoff_ms",
                &self.l1_batch_finalizer_retry_backoff_ms,
            )
            .field(
                "l1_batch_finalizer_max_attempts",
                &self.l1_batch_finalizer_max_attempts,
            )
            .field("da_max_payload_bytes", &self.da_max_payload_bytes)
            .field("da_public_backend", &self.da_public_backend)
            .field("da_public_fs_dir", &self.da_public_fs_dir)
            .field("da_public_base_url", &self.da_public_base_url)
            .field("mempool_replay_ttl_secs", &self.mempool_replay_ttl_secs)
            .field(
                "mempool_nonce_lock_ttl_secs",
                &self.mempool_nonce_lock_ttl_secs,
            )
            .field("mempool_leader_ttl_secs", &self.mempool_leader_ttl_secs)
            .field(
                "mempool_rate_limit_window_secs",
                &self.mempool_rate_limit_window_secs,
            )
            .field("mempool_max_global_queue", &self.mempool_max_global_queue)
            .field("mempool_max_account_queue", &self.mempool_max_account_queue)
            .field(
                "mempool_max_account_submissions_per_window",
                &self.mempool_max_account_submissions_per_window,
            )
            .field("mempool_max_payload_bytes", &self.mempool_max_payload_bytes)
            .field(
                "mempool_max_call_body_boc_base64_bytes",
                &self.mempool_max_call_body_boc_base64_bytes,
            )
            .field("mempool_min_gas_limit", &self.mempool_min_gas_limit)
            .field("mempool_max_gas_limit", &self.mempool_max_gas_limit)
            .field("mempool_min_gas_price", &self.mempool_min_gas_price)
            .field("mempool_max_tx_fee", &self.mempool_max_tx_fee)
            .field("mempool_pop_batch_size", &self.mempool_pop_batch_size)
            .field("executor_gas_schedule", &self.executor_gas_schedule)
            .finish()
    }
}
