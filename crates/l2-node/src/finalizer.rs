use crate::config::NodeConfig;
use crate::relayer::TonSubmitResult;
use async_trait::async_trait;
use l2_core::Hash32;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod provider;
mod service;
pub use provider::{RemoteFinalizeBatchSigner, ToncenterFinalizerProvider};
pub use service::BatchFinalizer;

pub(super) const FINALIZATION_LIMIT: u32 = 50;
pub(super) const SIGN_VALIDITY_SECONDS: u64 = 300;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchFinalizerConfig {
    pub chain_id: String,
    pub rollup_root_address: String,
    pub sender_address: String,
    pub finalize_msg_value_nanoton: u64,
    pub challenge_window_sec: u64,
    pub poll_interval_ms: u64,
    pub retry_backoff_ms: u64,
    pub max_attempts: u32,
}

impl BatchFinalizerConfig {
    pub fn from_node_config(config: &NodeConfig) -> Option<Self> {
        config.l1_batch_relayer_enabled.then(|| Self {
            chain_id: config.chain_id.clone(),
            rollup_root_address: config
                .l1_rollup_root_address
                .clone()
                .expect("validated finalizer config has root address"),
            sender_address: config
                .l1_sequencer_sender_address
                .clone()
                .expect("validated finalizer config has sender address"),
            finalize_msg_value_nanoton: config.l1_commit_msg_value_nanoton,
            challenge_window_sec: u64::from(config.challenge_window_sec),
            poll_interval_ms: config.l1_batch_relayer_poll_interval_ms,
            retry_backoff_ms: config.l1_batch_relayer_retry_backoff_ms,
            max_attempts: config.l1_batch_relayer_max_attempts,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalizerSignerOperation {
    FinalizeBatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FinalizeBatchSignRequest {
    pub operation: FinalizerSignerOperation,
    pub chain_id: String,
    pub rollup_root_address: String,
    pub sender_address: String,
    pub msg_value_nanoton: u64,
    pub batch_no: u64,
    pub valid_until: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedFinalizeBatch {
    pub boc_base64: String,
    pub signer_address: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OnchainBatchCommitment {
    pub exists: bool,
    pub committed_at: Option<u64>,
    pub finalized: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FinalizerStats {
    pub considered: usize,
    pub submitted: usize,
    pub finalized: usize,
    pub failed: usize,
    pub not_ready: usize,
    pub skipped: usize,
}

#[async_trait]
pub trait FinalizeBatchSigner: Send + Sync {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<SignedFinalizeBatch, FinalizerError>;
}

#[async_trait]
pub trait TonFinalizerProvider: Send + Sync {
    async fn send_signed_boc(
        &self,
        signed: &SignedFinalizeBatch,
    ) -> Result<TonSubmitResult, FinalizerError>;

    async fn message_confirmed(&self, message_hash: Hash32) -> Result<bool, FinalizerError>;

    async fn commitment(&self, batch_no: u64) -> Result<OnchainBatchCommitment, FinalizerError>;
}

pub trait FinalizerClock: Send + Sync {
    fn unix_time(&self) -> u64;
}

#[derive(Clone, Debug, Default)]
pub struct SystemFinalizerClock;

impl FinalizerClock for SystemFinalizerClock {
    fn unix_time(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
    }
}

#[derive(Debug, Error)]
pub enum FinalizerError {
    #[error("storage failed: {0}")]
    Storage(#[from] crate::storage::StorageError),
    #[error("ton finalizer HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("ton finalizer decoding failed: {0}")]
    Decode(&'static str),
    #[error("finalize signer failed: {0}")]
    Signer(String),
    #[error("ton finalizer provider failed: {0}")]
    Provider(String),
}

#[cfg(test)]
#[path = "finalizer_tests.rs"]
mod tests;
