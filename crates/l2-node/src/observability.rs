use crate::config::{NodeConfig, SecretString};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadinessReport {
    pub status: &'static str,
    pub components: BTreeMap<&'static str, ComponentReadiness>,
}

impl ReadinessReport {
    pub fn from_components(components: BTreeMap<&'static str, ComponentReadiness>) -> Self {
        let ready = components.values().all(|component| component.ready);
        Self {
            status: if ready { "ready" } else { "not_ready" },
            components,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComponentReadiness {
    pub ready: bool,
    pub code: &'static str,
    pub latency_ms: u64,
}

impl ComponentReadiness {
    pub fn ready(latency_ms: u64) -> Self {
        Self {
            ready: true,
            code: "ok",
            latency_ms,
        }
    }

    pub fn failed(code: &'static str, latency_ms: u64) -> Self {
        Self {
            ready: false,
            code,
            latency_ms,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct OperatorMetrics {
    pub block_production: BlockProductionMetrics,
    pub indexer: IndexerMetrics,
    pub relayer: RelayerMetrics,
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

    pub fn record_da_write_latency(&self, latency: Duration) {
        self.da_write_latency.record(latency);
    }

    pub fn record_storage_save_block_latency(&self, latency: Duration) {
        self.storage_save_block_latency.record(latency);
    }

    pub fn snapshot(&self) -> OperatorMetrics {
        let last_block_height_plus_one = self.last_block_height_plus_one.load(Ordering::Relaxed);
        OperatorMetrics {
            block_production: BlockProductionMetrics {
                attempts: self.block_attempts.load(Ordering::Relaxed),
                produced: self.blocks_produced.load(Ordering::Relaxed),
                empty: self.empty_blocks.load(Ordering::Relaxed),
                errors: self.block_errors.load(Ordering::Relaxed),
                last_height: last_block_height_plus_one.checked_sub(1),
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

pub async fn readiness_component<F, Fut>(code: &'static str, check: F) -> ComponentReadiness
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), ReadinessError>>,
{
    let started = Instant::now();
    match check().await {
        Ok(()) => ComponentReadiness::ready(duration_ms(started.elapsed())),
        Err(_) => ComponentReadiness::failed(code, duration_ms(started.elapsed())),
    }
}

#[async_trait]
pub trait TonReadinessProbe: Send + Sync {
    async fn check(&self) -> Result<(), ReadinessError>;
}

pub type DynTonReadinessProbe = Arc<dyn TonReadinessProbe>;

#[derive(Clone, Debug)]
pub struct ToncenterReadinessClient {
    base_url: String,
    api_key: SecretString,
    client: reqwest::Client,
}

impl ToncenterReadinessClient {
    pub fn from_config(config: &NodeConfig) -> Result<Self, ReadinessError> {
        Ok(Self {
            base_url: config
                .toncenter_v3_base_url
                .trim_end_matches('/')
                .to_owned(),
            api_key: config.toncenter_api_key.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()?,
        })
    }
}

#[async_trait]
impl TonReadinessProbe for ToncenterReadinessClient {
    async fn check(&self) -> Result<(), ReadinessError> {
        self.client
            .get(format!("{}/masterchainInfo", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReadyTonReadinessProbe;

#[async_trait]
impl TonReadinessProbe for ReadyTonReadinessProbe {
    async fn check(&self) -> Result<(), ReadinessError> {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ReadinessError {
    #[error("ton readiness HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("component is unavailable")]
    Unavailable,
}
