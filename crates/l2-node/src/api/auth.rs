use super::error::ApiError;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdminAuth {
    token: Option<String>,
}

impl AdminAuth {
    pub(crate) fn new(token: Option<String>) -> Self {
        Self {
            token: token.and_then(|token| {
                let token = token.trim().to_owned();
                (!token.is_empty()).then_some(token)
            }),
        }
    }

    pub(crate) fn authorize(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let Some(expected_token) = self.token.as_deref() else {
            return Err(ApiError::forbidden("admin api disabled"));
        };
        let Some(header_value) = headers.get(AUTHORIZATION) else {
            return Err(ApiError::unauthorized("missing admin bearer token"));
        };
        let header_value = header_value
            .to_str()
            .map_err(|_| ApiError::unauthorized("invalid authorization header"))?;
        let Some(actual_token) = header_value.strip_prefix("Bearer ") else {
            return Err(ApiError::unauthorized("missing admin bearer token"));
        };
        if !constant_time_eq(actual_token, expected_token) {
            return Err(ApiError::forbidden("invalid admin bearer token"));
        }

        Ok(())
    }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();

    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        diff |= (left_byte ^ right_byte) as usize;
    }

    diff == 0
}
