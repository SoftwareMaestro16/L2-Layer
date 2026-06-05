use super::duration_ms;
use crate::config::{NodeConfig, SecretString};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::BTreeMap;
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
