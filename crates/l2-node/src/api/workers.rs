use super::{ApiError, AppState};
use crate::config::NodeConfig;
use crate::da::DynDa;
use crate::finalizer::{
    BatchFinalizer, BatchFinalizerConfig, RemoteFinalizeBatchSigner, SystemFinalizerClock,
    ToncenterFinalizerProvider,
};
use crate::indexer::{DepositIndexerConfig, TonDepositIndexer, ToncenterClient};
use crate::observability::NodeMetrics;
use crate::relayer::{
    BatchRelayer, BatchRelayerConfig, RemoteCommitBatchSigner, ToncenterCommitProvider,
};
use crate::storage::DynStorage;
use l2_core::L2Block;
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};

pub(super) fn spawn_block_producer(state: AppState) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(2)).await;
            if let Err(error) = produce_block_once(&state).await {
                tracing::error!(?error, "failed to produce l2 block");
            }
        }
    });
}

pub(super) fn spawn_deposit_indexer(config: &NodeConfig, state: AppState) {
    let Some(indexer_config) = DepositIndexerConfig::from_node_config(config) else {
        return;
    };
    let poll_interval = Duration::from_millis(config.l1_deposit_poll_interval_ms);
    let indexer = TonDepositIndexer::new(indexer_config, ToncenterClient::from_config(config));
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match indexer.poll_once(&state.storage, &state.sequencer).await {
                Ok(stats) => {
                    state.metrics.record_indexer_poll(
                        stats.fetched,
                        stats.accepted,
                        stats.duplicates,
                    );
                    tracing::info!(
                        fetched = stats.fetched,
                        accepted = stats.accepted,
                        duplicates = stats.duplicates,
                        "ton deposit indexer poll completed"
                    );
                }
                Err(error) => {
                    state.metrics.record_indexer_error();
                    tracing::warn!(?error, "ton deposit indexer poll failed");
                }
            }
        }
    });
}

pub(super) fn spawn_batch_relayer(
    config: &NodeConfig,
    storage: DynStorage,
    da: DynDa,
    metrics: Arc<NodeMetrics>,
) {
    let Some(relayer_config) = BatchRelayerConfig::from_node_config(config) else {
        return;
    };
    let Some(signer) = RemoteCommitBatchSigner::from_config(config) else {
        tracing::error!("batch relayer enabled without signer config");
        return;
    };
    let poll_interval = Duration::from_millis(relayer_config.poll_interval_ms);
    let retry_backoff = Duration::from_millis(relayer_config.retry_backoff_ms);
    let relayer = BatchRelayer::new(
        relayer_config,
        storage,
        da,
        signer,
        ToncenterCommitProvider::from_config(config),
    );
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match relayer.relay_once().await {
                Ok(stats) => {
                    metrics.record_relayer_poll(
                        stats.considered,
                        stats.submitted,
                        stats.confirmed,
                        stats.failed,
                        stats.skipped,
                    );
                    tracing::info!(
                        considered = stats.considered,
                        submitted = stats.submitted,
                        confirmed = stats.confirmed,
                        failed = stats.failed,
                        skipped = stats.skipped,
                        "batch relayer poll completed"
                    );
                }
                Err(error) => {
                    metrics.record_relayer_error();
                    tracing::warn!(?error, "batch relayer poll failed");
                    sleep(retry_backoff).await;
                }
            }
        }
    });
}

pub(super) fn spawn_batch_finalizer(
    config: &NodeConfig,
    storage: DynStorage,
    metrics: Arc<NodeMetrics>,
) {
    let Some(finalizer_config) = BatchFinalizerConfig::from_node_config(config) else {
        return;
    };
    let Some(signer) = RemoteFinalizeBatchSigner::from_config(config) else {
        tracing::error!("batch finalizer enabled without signer config");
        return;
    };
    let poll_interval = Duration::from_millis(finalizer_config.poll_interval_ms);
    let retry_backoff = Duration::from_millis(finalizer_config.retry_backoff_ms);
    let finalizer = BatchFinalizer::new(
        finalizer_config,
        storage,
        signer,
        ToncenterFinalizerProvider::from_config(config),
        SystemFinalizerClock,
    );
    tokio::spawn(async move {
        loop {
            sleep(poll_interval).await;
            match finalizer.finalize_once().await {
                Ok(stats) => {
                    metrics.record_finalizer_poll(
                        stats.considered,
                        stats.submitted,
                        stats.finalized,
                        stats.failed,
                        stats.not_ready,
                        stats.skipped,
                    );
                    tracing::info!(
                        considered = stats.considered,
                        submitted = stats.submitted,
                        finalized = stats.finalized,
                        failed = stats.failed,
                        not_ready = stats.not_ready,
                        skipped = stats.skipped,
                        "batch finalizer poll completed"
                    );
                }
                Err(error) => {
                    metrics.record_finalizer_error();
                    tracing::warn!(?error, "batch finalizer poll failed");
                    sleep(retry_backoff).await;
                }
            }
        }
    });
}

pub(super) async fn produce_block_once(state: &AppState) -> Result<Option<L2Block>, ApiError> {
    const LEADER_OWNER: &str = "entropis-local-sequencer";
    state.metrics.record_block_attempt();
    if !state.mempool.acquire_leader_lock(LEADER_OWNER).await? {
        state.metrics.record_empty_block();
        return Ok(None);
    }

    let timestamp = current_unix_time();
    let result = async {
        let queued = state
            .mempool
            .pop_batch(state.mempool_pop_batch_size)
            .await?;
        let mut sequencer = state.sequencer.write().await;
        for tx in queued {
            sequencer.submit_tx(tx);
        }
        Ok::<_, ApiError>(sequencer.produce_block(timestamp))
    }
    .await;
    let _ = state.mempool.release_leader_lock(LEADER_OWNER).await;

    let Some(block) = (match result {
        Ok(block) => block,
        Err(error) => {
            state.metrics.record_block_error();
            return Err(error);
        }
    }) else {
        state.metrics.record_empty_block();
        return Ok(None);
    };

    tracing::info!(
        height = block.header.height,
        state_root = %block.header.state_root,
        "produced l2 block"
    );
    let da_started = Instant::now();
    let da_ref = state.da.write_batch_payload(&block).await?;
    state.metrics.record_da_write_latency(da_started.elapsed());
    tracing::info!(
        height = da_ref.block_height,
        data_hash = %da_ref.data_hash,
        payload_bytes = da_ref.payload_size,
        "published l2 batch data"
    );
    let storage_started = Instant::now();
    state.storage.save_block(block.clone()).await?;
    state
        .metrics
        .record_storage_save_block_latency(storage_started.elapsed());
    state.metrics.record_block_produced(block.header.height);
    Ok(Some(block))
}

fn current_unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
