use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct OperatorMetrics {
    pub block_production: BlockProductionMetrics,
    pub indexer: IndexerMetrics,
    pub relayer: RelayerMetrics,
    pub finalizer: FinalizerMetrics,
    pub latency: LatencyMetricsSnapshot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BlockProductionMetrics {
    pub attempts: u64,
    pub produced: u64,
    pub empty: u64,
    pub errors: u64,
    pub last_height: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct IndexerMetrics {
    pub polls: u64,
    pub errors: u64,
    pub fetched: u64,
    pub accepted: u64,
    pub duplicates: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RelayerMetrics {
    pub polls: u64,
    pub errors: u64,
    pub considered: u64,
    pub submitted: u64,
    pub confirmed: u64,
    pub failed: u64,
    pub skipped: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct FinalizerMetrics {
    pub polls: u64,
    pub errors: u64,
    pub considered: u64,
    pub submitted: u64,
    pub finalized: u64,
    pub failed: u64,
    pub not_ready: u64,
    pub skipped: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct LatencyMetricsSnapshot {
    pub da_write: LatencyMetricSnapshot,
    pub storage_save_block: LatencyMetricSnapshot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct LatencyMetricSnapshot {
    pub operations: u64,
    pub total_ms: u64,
    pub last_ms: u64,
    pub max_ms: u64,
}

#[derive(Debug, Default)]
pub struct NodeMetrics {
    block_attempts: AtomicU64,
    blocks_produced: AtomicU64,
    empty_blocks: AtomicU64,
    block_errors: AtomicU64,
    last_block_height_plus_one: AtomicU64,
    indexer_polls: AtomicU64,
    indexer_errors: AtomicU64,
    indexer_fetched: AtomicU64,
    indexer_accepted: AtomicU64,
    indexer_duplicates: AtomicU64,
    relayer_polls: AtomicU64,
    relayer_errors: AtomicU64,
    relayer_considered: AtomicU64,
    relayer_submitted: AtomicU64,
    relayer_confirmed: AtomicU64,
    relayer_failed: AtomicU64,
    relayer_skipped: AtomicU64,
    finalizer_polls: AtomicU64,
    finalizer_errors: AtomicU64,
    finalizer_considered: AtomicU64,
    finalizer_submitted: AtomicU64,
    finalizer_finalized: AtomicU64,
    finalizer_failed: AtomicU64,
    finalizer_not_ready: AtomicU64,
    finalizer_skipped: AtomicU64,
    da_write_latency: LatencyMetric,
    storage_save_block_latency: LatencyMetric,
}

impl NodeMetrics {
    pub fn record_block_attempt(&self) {
        self.block_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_block_produced(&self, height: u64) {
        self.blocks_produced.fetch_add(1, Ordering::Relaxed);
        self.last_block_height_plus_one
            .store(height.saturating_add(1), Ordering::Relaxed);
    }

    pub fn record_empty_block(&self) {
        self.empty_blocks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_block_error(&self) {
        self.block_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_indexer_poll(&self, fetched: usize, accepted: usize, duplicates: usize) {
        self.indexer_polls.fetch_add(1, Ordering::Relaxed);
        self.indexer_fetched
            .fetch_add(fetched as u64, Ordering::Relaxed);
        self.indexer_accepted
            .fetch_add(accepted as u64, Ordering::Relaxed);
        self.indexer_duplicates
            .fetch_add(duplicates as u64, Ordering::Relaxed);
    }

    pub fn record_indexer_error(&self) {
        self.indexer_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_relayer_poll(
        &self,
        considered: usize,
        submitted: usize,
        confirmed: usize,
        failed: usize,
        skipped: usize,
    ) {
        self.relayer_polls.fetch_add(1, Ordering::Relaxed);
        self.relayer_considered
            .fetch_add(considered as u64, Ordering::Relaxed);
        self.relayer_submitted
            .fetch_add(submitted as u64, Ordering::Relaxed);
        self.relayer_confirmed
            .fetch_add(confirmed as u64, Ordering::Relaxed);
        self.relayer_failed
            .fetch_add(failed as u64, Ordering::Relaxed);
        self.relayer_skipped
            .fetch_add(skipped as u64, Ordering::Relaxed);
    }

    pub fn record_relayer_error(&self) {
        self.relayer_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_finalizer_poll(
        &self,
        considered: usize,
        submitted: usize,
        finalized: usize,
        failed: usize,
        not_ready: usize,
        skipped: usize,
    ) {
        self.finalizer_polls.fetch_add(1, Ordering::Relaxed);
        self.finalizer_considered
            .fetch_add(considered as u64, Ordering::Relaxed);
        self.finalizer_submitted
            .fetch_add(submitted as u64, Ordering::Relaxed);
        self.finalizer_finalized
            .fetch_add(finalized as u64, Ordering::Relaxed);
        self.finalizer_failed
            .fetch_add(failed as u64, Ordering::Relaxed);
        self.finalizer_not_ready
            .fetch_add(not_ready as u64, Ordering::Relaxed);
        self.finalizer_skipped
            .fetch_add(skipped as u64, Ordering::Relaxed);
    }

    pub fn record_finalizer_error(&self) {
        self.finalizer_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_da_write_latency(&self, latency: Duration) {
        self.da_write_latency.record(latency);
    }

    pub fn record_storage_save_block_latency(&self, latency: Duration) {
        self.storage_save_block_latency.record(latency);
    }

    pub fn snapshot(&self) -> OperatorMetrics {
        let last = self.last_block_height_plus_one.load(Ordering::Relaxed);
        OperatorMetrics {
            block_production: BlockProductionMetrics {
                attempts: self.block_attempts.load(Ordering::Relaxed),
                produced: self.blocks_produced.load(Ordering::Relaxed),
                empty: self.empty_blocks.load(Ordering::Relaxed),
                errors: self.block_errors.load(Ordering::Relaxed),
                last_height: last.checked_sub(1),
            },
            indexer: IndexerMetrics {
                polls: self.indexer_polls.load(Ordering::Relaxed),
                errors: self.indexer_errors.load(Ordering::Relaxed),
                fetched: self.indexer_fetched.load(Ordering::Relaxed),
                accepted: self.indexer_accepted.load(Ordering::Relaxed),
                duplicates: self.indexer_duplicates.load(Ordering::Relaxed),
            },
            relayer: RelayerMetrics {
                polls: self.relayer_polls.load(Ordering::Relaxed),
                errors: self.relayer_errors.load(Ordering::Relaxed),
                considered: self.relayer_considered.load(Ordering::Relaxed),
                submitted: self.relayer_submitted.load(Ordering::Relaxed),
                confirmed: self.relayer_confirmed.load(Ordering::Relaxed),
                failed: self.relayer_failed.load(Ordering::Relaxed),
                skipped: self.relayer_skipped.load(Ordering::Relaxed),
            },
            finalizer: FinalizerMetrics {
                polls: self.finalizer_polls.load(Ordering::Relaxed),
                errors: self.finalizer_errors.load(Ordering::Relaxed),
                considered: self.finalizer_considered.load(Ordering::Relaxed),
                submitted: self.finalizer_submitted.load(Ordering::Relaxed),
                finalized: self.finalizer_finalized.load(Ordering::Relaxed),
                failed: self.finalizer_failed.load(Ordering::Relaxed),
                not_ready: self.finalizer_not_ready.load(Ordering::Relaxed),
                skipped: self.finalizer_skipped.load(Ordering::Relaxed),
            },
            latency: LatencyMetricsSnapshot {
                da_write: self.da_write_latency.snapshot(),
                storage_save_block: self.storage_save_block_latency.snapshot(),
            },
        }
    }
}

#[derive(Debug, Default)]
struct LatencyMetric {
    operations: AtomicU64,
    total_ms: AtomicU64,
    last_ms: AtomicU64,
    max_ms: AtomicU64,
}

impl LatencyMetric {
    fn record(&self, latency: Duration) {
        let ms = duration_ms(latency);
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.total_ms.fetch_add(ms, Ordering::Relaxed);
        self.last_ms.store(ms, Ordering::Relaxed);
        let mut current = self.max_ms.load(Ordering::Relaxed);
        while ms > current {
            match self
                .max_ms
                .compare_exchange(current, ms, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    fn snapshot(&self) -> LatencyMetricSnapshot {
        LatencyMetricSnapshot {
            operations: self.operations.load(Ordering::Relaxed),
            total_ms: self.total_ms.load(Ordering::Relaxed),
            last_ms: self.last_ms.load(Ordering::Relaxed),
            max_ms: self.max_ms.load(Ordering::Relaxed),
        }
    }
}

pub fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
