use super::error::ApiError;
use crate::config::NodeConfig;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct MempoolIngressGuard {
    config: Arc<MempoolIngressConfig>,
    windows: Arc<Mutex<BTreeMap<IpAddr, VecDeque<Instant>>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MempoolIngressConfig {
    window: Duration,
    max_submissions: u32,
    banned_ips: BTreeSet<IpAddr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MempoolIngressError {
    IpBanned,
    IpRateLimited,
}

impl MempoolIngressGuard {
    pub(crate) fn from_config(config: &NodeConfig) -> Self {
        Self {
            config: Arc::new(MempoolIngressConfig {
                window: Duration::from_secs(config.mempool_ip_rate_limit_window_secs),
                max_submissions: config.mempool_max_ip_submissions_per_window,
                banned_ips: config.mempool_banned_ips.iter().copied().collect(),
            }),
            windows: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn test(window: Duration, max_submissions: u32, banned_ips: Vec<IpAddr>) -> Self {
        Self {
            config: Arc::new(MempoolIngressConfig {
                window,
                max_submissions,
                banned_ips: banned_ips.into_iter().collect(),
            }),
            windows: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(crate) async fn check(&self, peer: SocketAddr) -> Result<(), MempoolIngressError> {
        let ip = peer.ip();
        if self.config.banned_ips.contains(&ip) {
            return Err(MempoolIngressError::IpBanned);
        }

        let mut windows = self.windows.lock().await;
        let now = Instant::now();
        let entries = windows.entry(ip).or_default();
        entries.retain(|timestamp| *timestamp + self.config.window > now);
        if entries.len() >= self.config.max_submissions as usize {
            return Err(MempoolIngressError::IpRateLimited);
        }
        entries.push_back(now);
        windows.retain(|_, entries| !entries.is_empty());
        Ok(())
    }
}

impl MempoolIngressError {
    pub(crate) fn reason_code(self) -> &'static str {
        match self {
            Self::IpBanned => "ip_banned",
            Self::IpRateLimited => "ip_rate_limited",
        }
    }
}

impl From<MempoolIngressError> for ApiError {
    fn from(error: MempoolIngressError) -> Self {
        match error {
            MempoolIngressError::IpBanned => ApiError::forbidden("ip_banned"),
            MempoolIngressError::IpRateLimited => ApiError::too_many_requests("ip_rate_limited"),
        }
    }
}
