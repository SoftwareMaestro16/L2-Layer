use super::NodeConfig;
use anyhow::anyhow;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeMode {
    LocalDev,
    TestnetPrototype,
}

impl RuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalDev => "local-dev",
            Self::TestnetPrototype => "testnet-prototype",
        }
    }

    pub(super) fn default_dev_admin_deposits(self) -> bool {
        matches!(self, Self::LocalDev)
    }

    pub(super) fn default_deposit_indexer(self) -> bool {
        matches!(self, Self::TestnetPrototype)
    }

    pub(super) fn default_batch_relayer(self) -> bool {
        matches!(self, Self::TestnetPrototype)
    }
}

pub(super) fn parse_runtime_mode(value: &str) -> anyhow::Result<RuntimeMode> {
    match value {
        "local-dev" => Ok(RuntimeMode::LocalDev),
        "testnet-prototype" => Ok(RuntimeMode::TestnetPrototype),
        _ => Err(anyhow!(
            "L2_RUNTIME_MODE must be local-dev or testnet-prototype"
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StartupSummary {
    pub runtime_mode: &'static str,
    pub chain_id: String,
    pub node_addr: String,
    pub ton_network: &'static str,
    pub toncenter_v3_base_url: String,
    pub tonapi_base_url: String,
    pub challenge_window_sec: u32,
    pub database_configured: bool,
    pub redis_configured: bool,
    pub dev_admin_deposits_enabled: bool,
    pub ent_faucet_require_admin: bool,
    pub l1_vault_address: Option<String>,
    pub l1_rollup_root_address: Option<String>,
    pub l1_deposit_indexer_enabled: bool,
    pub l1_batch_relayer_enabled: bool,
    pub l1_commit_signer_endpoint_configured: bool,
    pub da_max_payload_bytes: usize,
    pub mempool_max_global_queue: usize,
}

impl NodeConfig {
    pub fn startup_summary(&self) -> StartupSummary {
        StartupSummary {
            runtime_mode: self.runtime_mode.as_str(),
            chain_id: self.chain_id.clone(),
            node_addr: self.node_addr.to_string(),
            ton_network: "testnet",
            toncenter_v3_base_url: self.toncenter_v3_base_url.clone(),
            tonapi_base_url: self.tonapi_base_url.clone(),
            challenge_window_sec: self.challenge_window_sec,
            database_configured: true,
            redis_configured: true,
            dev_admin_deposits_enabled: self.dev_admin_deposits_enabled,
            ent_faucet_require_admin: self.ent_faucet_require_admin,
            l1_vault_address: self.l1_vault_address.clone(),
            l1_rollup_root_address: self.l1_rollup_root_address.clone(),
            l1_deposit_indexer_enabled: self.l1_deposit_indexer_enabled,
            l1_batch_relayer_enabled: self.l1_batch_relayer_enabled,
            l1_commit_signer_endpoint_configured: self.l1_commit_signer_endpoint.is_some(),
            da_max_payload_bytes: self.da_max_payload_bytes,
            mempool_max_global_queue: self.mempool_max_global_queue,
        }
    }
}
