use crate::da::DaError;
use crate::faucet::FaucetError;
use crate::mempool::MempoolError;
use crate::observer::ObserverError;
use crate::storage::StorageError;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: String,
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub(crate) fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    pub(crate) fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub(crate) fn gateway_timeout(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            message: message.into(),
        }
    }
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        tracing::error!(?error, "storage error");
        Self::internal("storage error")
    }
}

impl From<DaError> for ApiError {
    fn from(error: DaError) -> Self {
        tracing::error!(?error, "data availability error");
        Self::internal("data availability error")
    }
}

impl From<MempoolError> for ApiError {
    fn from(error: MempoolError) -> Self {
        if error.is_conflict() {
            return Self {
                status: StatusCode::CONFLICT,
                message: error.to_string(),
            };
        }
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
        }
    }
}

impl From<ObserverError> for ApiError {
    fn from(error: ObserverError) -> Self {
        match error {
            ObserverError::InvalidRequest(message) => Self::bad_request(message),
            ObserverError::CheckpointIntegrity => {
                Self::conflict("observer checkpoint failed integrity check")
            }
            other => {
                tracing::error!(?other, "observer replay error");
                Self::internal("observer replay error")
            }
        }
    }
}

impl From<FaucetError> for ApiError {
    fn from(error: FaucetError) -> Self {
        match error {
            FaucetError::InvalidAccountId
            | FaucetError::InvalidClaimId
            | FaucetError::InvalidAmount
            | FaucetError::ZeroAccountId => Self::bad_request(error.to_string()),
            FaucetError::Storage(storage_error) => storage_error.into(),
            FaucetError::AmountOverflow => Self::internal(error.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}
