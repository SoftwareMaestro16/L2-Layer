use super::{ApiError, AppState};
use crate::observability::{
    readiness_component, HealthResponse, OperatorMetrics, ReadinessError, ReadinessReport,
};
use crate::storage::{
    BatchCommitRecord, BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus,
};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Serialize;
use std::collections::BTreeMap;

const FAILURE_LIMIT: u32 = 50;
const VISIBILITY_LIMIT: u32 = 50;
const OPERATOR_MAX_ATTEMPTS_FILTER: u32 = i32::MAX as u32;

#[derive(Clone, Debug, Serialize)]
pub(super) struct OperatorMetricsResponse {
    pub node: OperatorMetrics,
    pub mempool: crate::mempool::MempoolMetrics,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct OperatorFailuresResponse {
    pub relayer_failed_batches: Vec<BatchCommitRecord>,
    pub failed_finalizations: Vec<BatchFinalizationRecord>,
    pub failed_withdrawals: FailedWithdrawalVisibility,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct BatchRelayerVisibilityResponse {
    pub latest: Option<BatchCommitRecord>,
    pub latest_confirmed: Option<BatchCommitRecord>,
    pub pending: Vec<BatchCommitRecord>,
    pub submitted: Vec<BatchCommitRecord>,
    pub failed: Vec<BatchCommitRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct BatchFinalizerVisibilityResponse {
    pub latest: Option<BatchFinalizationRecord>,
    pub latest_finalized: Option<BatchFinalizationRecord>,
    pub pending_finalization: Vec<BatchFinalizationRecord>,
    pub submitted_finalization: Vec<BatchFinalizationRecord>,
    pub failed_finalization: Vec<BatchFinalizationRecord>,
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
        .list_batch_commits(
            &[BatchCommitStatus::Failed],
            OPERATOR_MAX_ATTEMPTS_FILTER,
            FAILURE_LIMIT,
        )
        .await?;
    let failed_finalizations = state
        .storage
        .list_batch_finalizations(
            &[BatchFinalizationStatus::Failed],
            OPERATOR_MAX_ATTEMPTS_FILTER,
            FAILURE_LIMIT,
        )
        .await?;
    Ok(Json(OperatorFailuresResponse {
        relayer_failed_batches,
        failed_finalizations,
        failed_withdrawals: FailedWithdrawalVisibility {
            indexed: false,
            source: "RollupRoot.failedWithdrawal and AssetVault.failedRelease getters",
            runbook: "docs/operator-runbooks.md#withdrawal-release-failures",
        },
    }))
}

pub(super) async fn operator_batch_relayer(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BatchRelayerVisibilityResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    Ok(Json(BatchRelayerVisibilityResponse {
        latest: state.storage.latest_batch_commit(&[]).await?,
        latest_confirmed: state
            .storage
            .latest_batch_commit(&[BatchCommitStatus::Confirmed])
            .await?,
        pending: state
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Pending],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
        submitted: state
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Submitted],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
        failed: state
            .storage
            .list_batch_commits(
                &[BatchCommitStatus::Failed],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
    }))
}

pub(super) async fn operator_batch_finalizer(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<BatchFinalizerVisibilityResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    Ok(Json(BatchFinalizerVisibilityResponse {
        latest: state.storage.latest_batch_finalization(&[]).await?,
        latest_finalized: state
            .storage
            .latest_batch_finalization(&[BatchFinalizationStatus::Finalized])
            .await?,
        pending_finalization: state
            .storage
            .list_batch_finalizations(
                &[BatchFinalizationStatus::Pending],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
        submitted_finalization: state
            .storage
            .list_batch_finalizations(
                &[BatchFinalizationStatus::Submitted],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
        failed_finalization: state
            .storage
            .list_batch_finalizations(
                &[BatchFinalizationStatus::Failed],
                OPERATOR_MAX_ATTEMPTS_FILTER,
                VISIBILITY_LIMIT,
            )
            .await?,
    }))
}
