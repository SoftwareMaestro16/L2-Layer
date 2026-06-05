use crate::da::DynDa;
use crate::signer::BatchCommitment;
use crate::storage::{DynStorage, ObserverCheckpoint, StorageError};
use l2_core::{GasSchedule, Hash32, SequencerConfig, TvmAdapterMode, L2_NATIVE_GAS_ASSET};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

mod replay;

const MAX_REPLAY_COMMITMENTS: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverReplayConfig {
    pub chain_id: String,
    pub max_txs_per_block: usize,
    pub block_gas_limit: u64,
    pub gas_coin_asset: u32,
    pub gas_schedule: GasSchedule,
    pub max_internal_messages: u32,
    pub tvm_adapter_mode: TvmAdapterMode,
    pub tvm_tonlib_library_path: Option<PathBuf>,
}

impl ObserverReplayConfig {
    pub fn from_sequencer_config(config: &SequencerConfig) -> Self {
        Self {
            chain_id: config.chain_id.clone(),
            max_txs_per_block: config.max_txs_per_block,
            block_gas_limit: config.block_gas_limit,
            gas_coin_asset: config.gas_coin_asset,
            gas_schedule: config.gas_schedule,
            max_internal_messages: config.max_internal_messages,
            tvm_adapter_mode: config.tvm_adapter_mode.clone(),
            tvm_tonlib_library_path: config.tvm_tonlib_library_path.clone(),
        }
    }
}

impl Default for ObserverReplayConfig {
    fn default() -> Self {
        Self {
            chain_id: "entropis-testnet".to_owned(),
            max_txs_per_block: 1024,
            block_gas_limit: 1_000_000,
            gas_coin_asset: L2_NATIVE_GAS_ASSET,
            gas_schedule: GasSchedule::default(),
            max_internal_messages: 1024,
            tvm_adapter_mode: TvmAdapterMode::Real,
            tvm_tonlib_library_path: None,
        }
    }
}

#[derive(Clone)]
pub struct ObserverReplayService {
    storage: DynStorage,
    da: DynDa,
    config: ObserverReplayConfig,
}

impl ObserverReplayService {
    pub fn new(storage: DynStorage, da: DynDa, config: ObserverReplayConfig) -> Self {
        Self {
            storage,
            da,
            config,
        }
    }

    pub async fn latest_checkpoint(&self) -> Result<Option<ObserverCheckpoint>, ObserverError> {
        Ok(self.storage.latest_observer_checkpoint().await?)
    }

    pub async fn replay(
        &self,
        request: ObserverReplayRequest,
    ) -> Result<ObserverReplayReport, ObserverError> {
        if request.commitments.len() > MAX_REPLAY_COMMITMENTS {
            return Err(ObserverError::InvalidRequest(
                "observer replay range is too large",
            ));
        }
        let mut checkpoint = match request.trusted_checkpoint {
            Some(checkpoint) => checkpoint,
            None => self
                .storage
                .latest_observer_checkpoint()
                .await?
                .unwrap_or_else(ObserverCheckpoint::genesis),
        };
        if !checkpoint.validate_integrity() {
            return Err(ObserverError::CheckpointIntegrity);
        }

        let mut checked_batches = 0u64;
        for commitment in request.commitments {
            if let Some(divergence) = replay::validate_commitment_order(&checkpoint, &commitment) {
                return Ok(ObserverReplayReport::diverged(
                    ObserverReplayStatus::Invalid,
                    checked_batches,
                    checkpoint,
                    divergence,
                ));
            }
            let outcome =
                replay::replay_commitment(&self.config, self.da.as_ref(), &checkpoint, &commitment)
                    .await?;
            let Some(next_checkpoint) = outcome.next_checkpoint else {
                return Ok(ObserverReplayReport::diverged(
                    outcome.status,
                    checked_batches,
                    checkpoint,
                    outcome.divergence.expect("divergent outcome has details"),
                ));
            };
            checkpoint = next_checkpoint;
            checked_batches = checked_batches.saturating_add(1);
            if request.store_checkpoint {
                self.storage
                    .save_observer_checkpoint(checkpoint.clone())
                    .await?;
            }
        }

        Ok(ObserverReplayReport {
            status: ObserverReplayStatus::Valid,
            checked_batches,
            latest_checkpoint: checkpoint,
            first_divergence: None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObserverReplayRequest {
    #[serde(default)]
    pub trusted_checkpoint: Option<ObserverCheckpoint>,
    pub commitments: Vec<BatchCommitment>,
    #[serde(default)]
    pub store_checkpoint: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObserverReplayReport {
    pub status: ObserverReplayStatus,
    pub checked_batches: u64,
    pub latest_checkpoint: ObserverCheckpoint,
    pub first_divergence: Option<ObserverDivergence>,
}

impl ObserverReplayReport {
    fn diverged(
        status: ObserverReplayStatus,
        checked_batches: u64,
        latest_checkpoint: ObserverCheckpoint,
        divergence: ObserverDivergence,
    ) -> Self {
        Self {
            status,
            checked_batches,
            latest_checkpoint,
            first_divergence: Some(divergence),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverReplayStatus {
    Valid,
    Invalid,
    MissingDa,
    CorruptDa,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObserverDivergence {
    pub batch_no: u64,
    pub block_height: u64,
    pub kind: DivergenceKind,
    pub field: Option<&'static str>,
    pub tx_index: Option<usize>,
    pub expected_hash: Option<Hash32>,
    pub actual_hash: Option<Hash32>,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    NonContiguousCommitment,
    MissingDa,
    CorruptDa,
    ReceiptMismatch,
    RootMismatch,
}

#[derive(Debug, Error)]
pub enum ObserverError {
    #[error("observer request is invalid: {0}")]
    InvalidRequest(&'static str),
    #[error("observer checkpoint failed integrity check")]
    CheckpointIntegrity,
    #[error("observer withdrawal root failed: {0}")]
    WithdrawalRoot(#[from] l2_core::WithdrawalProofError),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("data availability failed: {0}")]
    DataAvailability(#[from] crate::da::DaError),
}

#[cfg(test)]
#[path = "observer_tests.rs"]
mod tests;
