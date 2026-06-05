use super::{NodeConfig, TonNetwork};
use anyhow::anyhow;

impl NodeConfig {
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        if self.ton_network != TonNetwork::Testnet {
            return Err(anyhow!("only TON testnet is allowed for this node"));
        }
        if self.chain_id != super::DEFAULT_CHAIN_ID {
            return Err(anyhow!("L2_CHAIN_ID must be {}", super::DEFAULT_CHAIN_ID));
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
        if self.native_token_symbol != super::DEFAULT_TOKEN_SYMBOL {
            return Err(anyhow!(
                "L2_NATIVE_TOKEN_SYMBOL must be {}",
                super::DEFAULT_TOKEN_SYMBOL
            ));
        }
        if self.ent_faucet_amount == 0 {
            return Err(anyhow!("ENT_FAUCET_AMOUNT must be non-zero"));
        }
        if self.ent_decimals != super::DEFAULT_ENT_DECIMALS {
            return Err(anyhow!(
                "ENT_DECIMALS must be {}",
                super::DEFAULT_ENT_DECIMALS
            ));
        }
        if !super::path_exists_in_cwd_or_ancestors(&self.ent_logo_path) {
            return Err(anyhow!("ENT_LOGO_PATH must point to an existing file"));
        }
        if !self.ent_faucet_require_admin {
            return Err(anyhow!("ENT_FAUCET_REQUIRE_ADMIN must be true for MVP"));
        }
        self.validate_l1_indexer()?;
        self.validate_l1_relayer()?;
        self.validate_l1_finalizer()?;
        self.validate_da()?;
        self.validate_tvm_getters()?;
        self.validate_mempool()?;
        self.executor_gas_schedule
            .validate()
            .map_err(|error| anyhow!("invalid executor gas schedule: {error}"))?;
        Ok(())
    }

    fn validate_l1_indexer(&self) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn validate_l1_relayer(&self) -> anyhow::Result<()> {
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

    fn validate_l1_finalizer(&self) -> anyhow::Result<()> {
        if self.l1_batch_finalizer_enabled {
            if self.l1_rollup_root_address.is_none() {
                return Err(anyhow!(
                    "L1_ROLLUP_ROOT_ADDRESS is required when L1_BATCH_FINALIZER_ENABLED=true"
                ));
            }
            if self.l1_sequencer_sender_address.is_none() {
                return Err(anyhow!(
                    "L1_SEQUENCER_SENDER_ADDRESS is required when L1_BATCH_FINALIZER_ENABLED=true"
                ));
            }
            if self.l1_finalize_signer_endpoint.is_none() {
                return Err(anyhow!(
                    "L1_FINALIZE_SIGNER_ENDPOINT is required when L1_BATCH_FINALIZER_ENABLED=true"
                ));
            }
            if self.l1_finalize_signer_token.is_none() {
                return Err(anyhow!(
                    "L1_FINALIZE_SIGNER_TOKEN is required when L1_BATCH_FINALIZER_ENABLED=true"
                ));
            }
        }
        if let Some(endpoint) = self.l1_finalize_signer_endpoint.as_deref() {
            if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                return Err(anyhow!("L1_FINALIZE_SIGNER_ENDPOINT must be an HTTP URL"));
            }
        }
        if self.l1_finalize_msg_value_nanoton == 0 {
            return Err(anyhow!("L1_FINALIZE_MSG_VALUE_NANOTON must be non-zero"));
        }
        if self.l1_batch_finalizer_poll_interval_ms == 0 {
            return Err(anyhow!(
                "L1_BATCH_FINALIZER_POLL_INTERVAL_MS must be non-zero"
            ));
        }
        if self.l1_batch_finalizer_retry_backoff_ms == 0 {
            return Err(anyhow!(
                "L1_BATCH_FINALIZER_RETRY_BACKOFF_MS must be non-zero"
            ));
        }
        if self.l1_batch_finalizer_max_attempts == 0 || self.l1_batch_finalizer_max_attempts > 100 {
            return Err(anyhow!(
                "L1_BATCH_FINALIZER_MAX_ATTEMPTS must be between 1 and 100"
            ));
        }
        Ok(())
    }

    fn validate_da(&self) -> anyhow::Result<()> {
        if self.da_max_payload_bytes == 0 {
            return Err(anyhow!("DA_MAX_PAYLOAD_BYTES must be non-zero"));
        }
        if self.da_max_payload_bytes > 128 * 1024 * 1024 {
            return Err(anyhow!("DA_MAX_PAYLOAD_BYTES must not exceed 128 MiB"));
        }
        match self.da_public_backend.as_str() {
            "postgres" => {}
            "filesystem" => {
                if self.da_public_fs_dir.as_os_str().is_empty() {
                    return Err(anyhow!("DA_PUBLIC_FS_DIR must be non-empty"));
                }
            }
            _ => {
                return Err(anyhow!("DA_PUBLIC_BACKEND must be postgres or filesystem"));
            }
        }
        if let Some(base_url) = self.da_public_base_url.as_deref() {
            if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
                return Err(anyhow!("DA_PUBLIC_BASE_URL must be an HTTP URL"));
            }
        }
        Ok(())
    }

    fn validate_tvm_getters(&self) -> anyhow::Result<()> {
        if self.tvm_getter_default_gas_limit == 0 || self.tvm_getter_max_gas_limit == 0 {
            return Err(anyhow!("TVM getter gas limits must be non-zero"));
        }
        if self.tvm_getter_default_gas_limit > self.tvm_getter_max_gas_limit {
            return Err(anyhow!(
                "TVM_GETTER_DEFAULT_GAS_LIMIT must be <= TVM_GETTER_MAX_GAS_LIMIT"
            ));
        }
        if self.tvm_getter_timeout_ms == 0 {
            return Err(anyhow!("TVM_GETTER_TIMEOUT_MS must be non-zero"));
        }
        if self.tvm_getter_max_stack_boc_bytes == 0
            || self.tvm_getter_max_stack_boc_bytes > super::DEFAULT_MEMPOOL_MAX_PAYLOAD_BYTES
        {
            return Err(anyhow!(
                "TVM_GETTER_MAX_STACK_BOC_BYTES must be between 1 and MEMPOOL_MAX_PAYLOAD_BYTES"
            ));
        }
        Ok(())
    }

    fn validate_mempool(&self) -> anyhow::Result<()> {
        if self.mempool_replay_ttl_secs == 0
            || self.mempool_nonce_lock_ttl_secs == 0
            || self.mempool_leader_ttl_secs == 0
            || self.mempool_rate_limit_window_secs == 0
            || self.mempool_ip_rate_limit_window_secs == 0
        {
            return Err(anyhow!("mempool TTL/window values must be non-zero"));
        }
        if self.mempool_max_global_queue == 0
            || self.mempool_max_account_queue == 0
            || self.mempool_max_account_nonce_window == 0
            || self.mempool_max_account_submissions_per_window == 0
            || self.mempool_max_ip_submissions_per_window == 0
            || self.mempool_max_payload_bytes == 0
            || self.mempool_max_transfer_payload_bytes == 0
            || self.mempool_max_withdraw_payload_bytes == 0
            || self.mempool_max_call_payload_bytes == 0
            || self.mempool_max_deploy_payload_bytes == 0
            || self.mempool_max_call_body_boc_base64_bytes == 0
            || self.mempool_pop_batch_size == 0
        {
            return Err(anyhow!("mempool limits must be non-zero"));
        }
        if self.mempool_max_account_queue > self.mempool_max_global_queue {
            return Err(anyhow!(
                "MEMPOOL_MAX_ACCOUNT_QUEUE must not exceed MEMPOOL_MAX_GLOBAL_QUEUE"
            ));
        }
        if self.mempool_min_gas_limit == 0
            || self.mempool_min_gas_limit > self.mempool_max_gas_limit
        {
            return Err(anyhow!(
                "MEMPOOL_MIN_GAS_LIMIT must be non-zero and <= MEMPOOL_MAX_GAS_LIMIT"
            ));
        }
        if self.mempool_min_gas_price == 0 || self.mempool_max_tx_fee == 0 {
            return Err(anyhow!(
                "MEMPOOL_MIN_GAS_PRICE and MEMPOOL_MAX_TX_FEE must be non-zero"
            ));
        }
        Ok(())
    }
}
