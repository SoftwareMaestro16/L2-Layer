use super::{ApiError, AppState};
use crate::da::DaError;
use crate::observability::duration_ms;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use l2_core::Hash32;
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Debug, Serialize)]
pub(super) struct DaPayloadVisibilityResponse {
    pub status: &'static str,
    pub block_height: u64,
    pub data_hash: Hash32,
    pub block_hash: Option<Hash32>,
    pub payload_size: Option<usize>,
    pub public_ref: Option<String>,
    pub public_uri: Option<String>,
    pub download_path: Option<String>,
    pub latency_ms: u64,
    pub reason: Option<&'static str>,
}

pub(super) async fn operator_da_payload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((height, data_hash)): Path<(u64, String)>,
) -> Result<Json<DaPayloadVisibilityResponse>, ApiError> {
    state.admin_auth.authorize(&headers)?;
    let data_hash =
        Hash32::from_hex(&data_hash).map_err(|_| ApiError::bad_request("invalid data hash"))?;
    let started = Instant::now();

    let Some(block) = state.storage.get_block(height).await? else {
        return Ok(Json(missing_block(height, data_hash, started)));
    };

    let block_hash = block.header.block_hash();
    if block.header.data_hash != data_hash {
        return Ok(Json(DaPayloadVisibilityResponse {
            status: "corrupt",
            block_height: height,
            data_hash,
            block_hash: Some(block_hash),
            payload_size: None,
            public_ref: None,
            public_uri: None,
            download_path: None,
            latency_ms: duration_ms(started.elapsed()),
            reason: Some("data_hash_not_for_block"),
        }));
    }

    match state.da.verify_batch_payload(&block).await {
        Ok(da_ref) => Ok(Json(DaPayloadVisibilityResponse {
            status: "available",
            block_height: height,
            data_hash,
            block_hash: Some(da_ref.block_hash),
            payload_size: Some(da_ref.payload_size),
            public_ref: da_ref.public_ref,
            public_uri: da_ref.public_uri,
            download_path: Some(format!("/v1/da/batch/{height}/{}", data_hash.to_hex())),
            latency_ms: duration_ms(started.elapsed()),
            reason: None,
        })),
        Err(error) => Ok(Json(DaPayloadVisibilityResponse {
            status: da_status_for_error(&error),
            block_height: height,
            data_hash,
            block_hash: Some(block_hash),
            payload_size: None,
            public_ref: None,
            public_uri: None,
            download_path: None,
            latency_ms: duration_ms(started.elapsed()),
            reason: Some(da_reason_for_error(&error)),
        })),
    }
}

fn missing_block(height: u64, data_hash: Hash32, started: Instant) -> DaPayloadVisibilityResponse {
    DaPayloadVisibilityResponse {
        status: "missing",
        block_height: height,
        data_hash,
        block_hash: None,
        payload_size: None,
        public_ref: None,
        public_uri: None,
        download_path: None,
        latency_ms: duration_ms(started.elapsed()),
        reason: Some("l2_block_missing"),
    }
}

fn da_status_for_error(error: &DaError) -> &'static str {
    match error {
        DaError::Unavailable => "missing",
        DaError::Storage(_) | DaError::PublicIo(_) => "unavailable",
        DaError::PayloadTooLarge { .. }
        | DaError::HashMismatch { .. }
        | DaError::BlockHashMismatch { .. }
        | DaError::InvalidPublicReference
        | DaError::AmbiguousPublicPayload => "corrupt",
    }
}

fn da_reason_for_error(error: &DaError) -> &'static str {
    match error {
        DaError::Unavailable => "batch_data_unavailable",
        DaError::PayloadTooLarge { .. } => "batch_data_oversized",
        DaError::HashMismatch { .. } => "batch_data_hash_mismatch",
        DaError::BlockHashMismatch { .. } => "batch_block_hash_mismatch",
        DaError::InvalidPublicReference => "batch_public_reference_invalid",
        DaError::AmbiguousPublicPayload => "batch_public_reference_ambiguous",
        DaError::PublicIo(_) => "public_da_io_failed",
        DaError::Storage(_) => "storage_error",
    }
}
