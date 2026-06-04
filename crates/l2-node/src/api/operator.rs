use super::{ApiError, AppState};
use crate::observability::{
    readiness_component, HealthResponse, OperatorMetrics, ReadinessError, ReadinessReport,
};
use crate::storage::{BatchCommitRecord, BatchCommitStatus};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Serialize;
use std::collections::BTreeMap;

const FAILURE_LIMIT: u32 = 50;

#[derive(Clone, Debug, Serialize)]
pub(super) struct OperatorMetricsResponse {
    pub node: OperatorMetrics,
    pub mempool: crate::mempool::MempoolMetrics,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct OperatorFailuresResponse {
    pub relayer_failed_batches: Vec<BatchCommitRecord>,
    pub failed_withdrawals: FailedWithdrawalVisibility,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct FailedWithdrawalVisibility {
    pub indexed: bool,
    pub source: &'static str,
    pub runbook: &'static str,
}

pub(super) async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "alive",
        service: "entropis-l2-node",
    })
}

pub(super) async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<ReadinessReport>) {
    let mut components = BTreeMap::new();
    components.insert(
        "db",
        readiness_component("db_unavailable", || async {
            state
                .storage
                .health_check()
                .await
                .map_err(|_| ReadinessError::Unavailable)
        })
        .await,
    );
    components.insert(
        "redis",
        readiness_component("redis_unavailable", || async {
            state
                .mempool
                .health_check()
                .await
                .map_err(|_| ReadinessError::Unavailable)
        })
        .await,
    );
    components.insert(
        "ton",
        readiness_component("ton_unavailable", || async {
            state.ton_readiness.check().await
        })
        .await,
    );

    let report = ReadinessReport::from_components(components);
    let status = if report.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report))
}

pub(super) async fn operator_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OperatorMetricsResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    Ok(Json(OperatorMetricsResponse {
        node: state.metrics.snapshot(),
        mempool: state.mempool.metrics().await?,
    }))
}

pub(super) async fn operator_failures(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OperatorFailuresResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let relayer_failed_batches = state
        .storage
        .list_batch_commits(&[BatchCommitStatus::Failed], u32::MAX, FAILURE_LIMIT)
        .await?;
    Ok(Json(OperatorFailuresResponse {
        relayer_failed_batches,
        failed_withdrawals: FailedWithdrawalVisibility {
            indexed: false,
            source: "RollupRoot.failedWithdrawal and AssetVault.failedRelease getters",
            runbook: "docs/operator-runbooks.md#withdrawal-release-failures",
        },
    }))
}
