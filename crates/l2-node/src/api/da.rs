use super::{ApiError, AppState};
use crate::da::DaError;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::IntoResponse;
use l2_core::Hash32;

pub(super) async fn get_batch_da_payload(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<impl IntoResponse, ApiError> {
    let payload = state
        .da
        .read_batch_payload(height)
        .await?
        .ok_or_else(|| ApiError::not_found("batch data not found"))?;
    batch_payload_response(payload)
}

pub(super) async fn get_batch_da_payload_by_hash(
    State(state): State<AppState>,
    Path((height, data_hash)): Path<(u64, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let data_hash =
        Hash32::from_hex(&data_hash).map_err(|_| ApiError::bad_request("invalid data hash"))?;
    let payload = state
        .da
        .read_batch_payload_by_hash(height, data_hash)
        .await?
        .ok_or_else(|| ApiError::not_found("batch data not found"))?;
    batch_payload_response(payload)
}

fn batch_payload_response(
    payload: crate::storage::StoredBatchPayload,
) -> Result<impl IntoResponse, ApiError> {
    let actual = l2_core::crypto::hash_domain("l2.batch.data.v1", &[&payload.payload_bytes]);
    if actual != payload.data_hash {
        return Err(DaError::HashMismatch {
            expected: payload.data_hash,
            actual,
        }
        .into());
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        "x-entropis-block-height",
        HeaderValue::from_str(&payload.block_height.to_string())
            .expect("u64 is a valid header value"),
    );
    headers.insert(
        "x-entropis-block-hash",
        HeaderValue::from_str(&payload.block_hash.to_hex()).expect("hash hex is valid header"),
    );
    headers.insert(
        "x-entropis-data-hash",
        HeaderValue::from_str(&payload.data_hash.to_hex()).expect("hash hex is valid header"),
    );
    if let Some(public_ref) = payload.public_ref.as_deref() {
        headers.insert(
            "x-entropis-da-ref",
            HeaderValue::from_str(public_ref)
                .map_err(|_| ApiError::internal("invalid batch data reference"))?,
        );
    }
    if let Some(public_uri) = payload.public_uri.as_deref() {
        headers.insert(
            "x-entropis-da-uri",
            HeaderValue::from_str(public_uri)
                .map_err(|_| ApiError::internal("invalid batch data uri"))?,
        );
    }
    Ok((headers, payload.payload_bytes))
}
