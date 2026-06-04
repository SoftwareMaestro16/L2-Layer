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
const BATCH_COMMIT_LIMIT: u32 = 50;

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
pub(super) struct OperatorBatchCommitsResponse {
    pub batch_commits: Vec<BatchCommitRecord>,
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

pub(super) async fn operator_batch_commits(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OperatorBatchCommitsResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let batch_commits = state
        .storage
        .list_batch_commits(
            &[
                BatchCommitStatus::Pending,
                BatchCommitStatus::Submitted,
                BatchCommitStatus::Confirmed,
                BatchCommitStatus::Failed,
            ],
            u32::MAX,
            BATCH_COMMIT_LIMIT,
        )
        .await?;
    Ok(Json(OperatorBatchCommitsResponse { batch_commits }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::AUTHORIZATION;
    use axum::http::HeaderValue;
    use l2_core::{canonical_batch_data_hash, crypto::sha256_bytes, Hash32, L2Block};

    const ADMIN_TOKEN: &str = "test-admin-token";

    #[tokio::test]
    async fn operator_batch_commits_requires_admin_and_lists_statuses() {
        let unauthorized =
            operator_batch_commits(State(AppState::test(None)), auth_headers(ADMIN_TOKEN))
                .await
                .unwrap_err();
        assert_eq!(unauthorized.status, StatusCode::FORBIDDEN);

        let state = AppState::test(Some(ADMIN_TOKEN));
        state.storage.save_block(empty_block(0)).await.unwrap();
        let mut record = state.storage.get_batch_commit(1).await.unwrap().unwrap();
        record.status = BatchCommitStatus::Submitted;
        record.attempts = 1;
        record.message_hash_norm = Some(sha256_bytes(b"norm"));
        state.storage.save_batch_commit(record).await.unwrap();

        let response = operator_batch_commits(State(state), auth_headers(ADMIN_TOKEN))
            .await
            .expect("batch commits");

        assert_eq!(response.0.batch_commits.len(), 1);
        assert_eq!(
            response.0.batch_commits[0].status,
            BatchCommitStatus::Submitted
        );
        assert_eq!(response.0.batch_commits[0].attempts, 1);
        assert!(response.0.batch_commits[0].message_hash_norm.is_some());
    }

    fn auth_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("valid header"),
        );
        headers
    }

    fn empty_block(height: u64) -> L2Block {
        L2Block::new(
            height,
            Hash32::ZERO,
            Hash32::ZERO,
            sha256_bytes(b"state"),
            vec![],
            vec![],
            vec![],
            canonical_batch_data_hash(&[], &[]),
            100,
        )
    }
}
