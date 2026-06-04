use crate::config::SecretString;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::backend::{SignerBackendError, TypedSignerBackend};
use super::types::{
    unix_time, SignedExternalMessage, SignerRole, TypedSignAction, TypedSignRequest,
};

#[derive(Clone, Debug)]
pub struct SignerServiceConfig {
    pub token: SecretString,
    pub signer_address: String,
    pub role: SignerRole,
    pub max_body_bytes: usize,
    pub rate_limit_per_minute: u32,
}

impl SignerServiceConfig {
    pub fn validate(&self) -> Result<(), SignerConfigError> {
        if self.signer_address.trim().is_empty() {
            return Err(SignerConfigError::MissingSignerAddress);
        }
        if self.max_body_bytes == 0 {
            return Err(SignerConfigError::InvalidBodyLimit);
        }
        if self.rate_limit_per_minute == 0 {
            return Err(SignerConfigError::InvalidRateLimit);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SignerServiceState<B> {
    config: Arc<SignerServiceConfig>,
    backend: B,
    limiter: Arc<SignerRateLimiter>,
}

pub fn build_signer_router<B>(config: SignerServiceConfig, backend: B) -> Router
where
    B: TypedSignerBackend + 'static,
{
    config.validate().expect("validated signer config");
    let max_body_bytes = config.max_body_bytes;
    let limiter = SignerRateLimiter::new(config.rate_limit_per_minute, Duration::from_secs(60));
    let state = SignerServiceState {
        config: Arc::new(config),
        backend,
        limiter: Arc::new(limiter),
    };
    Router::new()
        .route("/sign", post(sign_typed::<B>))
        .route("/sign-commit", post(sign_typed::<B>))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .with_state(state)
}

async fn sign_typed<B>(
    State(state): State<SignerServiceState<B>>,
    headers: HeaderMap,
    Json(request): Json<TypedSignRequest>,
) -> Result<Json<SignedExternalMessage>, SignerHttpError>
where
    B: TypedSignerBackend + 'static,
{
    authorize(&headers, state.config.token.expose())?;
    if !state.limiter.check().await {
        return Err(SignerHttpError::RateLimited);
    }
    let now = unix_time();
    request.validate(now)?;
    if request.role != state.config.role {
        return Err(SignerHttpError::Forbidden("signer_role_mismatch"));
    }
    if !matches!(request.action, TypedSignAction::CommitBatch(_)) {
        return Err(SignerHttpError::UnsupportedAction);
    }

    let request_id = request.request_id.clone();
    let action = request.action.action();
    let signed = state.backend.sign(request).await?;
    if signed.signer_address != state.config.signer_address {
        return Err(SignerHttpError::BadBackendResponse(
            "signer_address_mismatch",
        ));
    }
    signed.validate(unix_time(), state.config.max_body_bytes)?;
    Ok(Json(SignedExternalMessage {
        request_id,
        action,
        boc_base64: signed.boc_base64,
        signer_address: signed.signer_address,
        valid_until: signed.valid_until,
    }))
}

#[derive(Debug, Error)]
pub enum SignerConfigError {
    #[error("missing signer address")]
    MissingSignerAddress,
    #[error("invalid signer body limit")]
    InvalidBodyLimit,
    #[error("invalid signer rate limit")]
    InvalidRateLimit,
    #[error("invalid signer role")]
    InvalidRole,
}

#[derive(Debug)]
enum SignerHttpError {
    Unauthorized,
    Forbidden(&'static str),
    RateLimited,
    UnsupportedAction,
    BadRequest(&'static str),
    BadBackendResponse(&'static str),
    Backend(&'static str),
}

impl IntoResponse for SignerHttpError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::UnsupportedAction => StatusCode::UNPROCESSABLE_ENTITY,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::BadBackendResponse(_) | Self::Backend(_) => StatusCode::BAD_GATEWAY,
        };
        let error = match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden(code)
            | Self::BadRequest(code)
            | Self::BadBackendResponse(code)
            | Self::Backend(code) => code,
            Self::RateLimited => "rate_limited",
            Self::UnsupportedAction => "unsupported_action",
        };
        (status, Json(SignerErrorBody { error })).into_response()
    }
}

impl From<super::types::SignerValidationError> for SignerHttpError {
    fn from(error: super::types::SignerValidationError) -> Self {
        Self::BadRequest(error.safe_code())
    }
}

impl From<SignerBackendError> for SignerHttpError {
    fn from(error: SignerBackendError) -> Self {
        Self::Backend(error.safe_code())
    }
}

#[derive(Serialize)]
struct SignerErrorBody {
    error: &'static str,
}

struct SignerRateLimiter {
    max_requests: usize,
    window: Duration,
    hits: Mutex<VecDeque<Instant>>,
}

impl SignerRateLimiter {
    fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests: max_requests as usize,
            window,
            hits: Mutex::new(VecDeque::new()),
        }
    }

    async fn check(&self) -> bool {
        let now = Instant::now();
        let mut hits = self.hits.lock().await;
        while hits
            .front()
            .is_some_and(|hit| now.duration_since(*hit) >= self.window)
        {
            hits.pop_front();
        }
        if hits.len() >= self.max_requests {
            return false;
        }
        hits.push_back(now);
        true
    }
}

fn authorize(headers: &HeaderMap, expected_token: &str) -> Result<(), SignerHttpError> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(SignerHttpError::Unauthorized);
    };
    let Ok(value) = value.to_str() else {
        return Err(SignerHttpError::Unauthorized);
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(SignerHttpError::Unauthorized);
    };
    secure_eq(token.as_bytes(), expected_token.as_bytes())
        .then_some(())
        .ok_or(SignerHttpError::Unauthorized)
}

fn secure_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let a = left.get(index).copied().unwrap_or_default();
        let b = right.get(index).copied().unwrap_or_default();
        diff |= (a ^ b) as usize;
    }
    diff == 0
}
