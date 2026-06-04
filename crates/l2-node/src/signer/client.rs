use crate::config::{NodeConfig, SecretString};
use async_trait::async_trait;
use thiserror::Error;

use super::types::{
    commit_request_id, finalize_request_id, unix_time, CommitBatchSignRequest,
    FinalizeBatchSignRequest, SignedCommitBatch, SignedExternalMessage, SignedFinalizeBatch,
    SignerValidationError, TypedSignRequest, DEFAULT_SIGNER_MAX_BODY_BYTES,
    DEFAULT_SIGNER_VALIDITY_SECS,
};

#[async_trait]
pub trait CommitBatchSigner: Send + Sync {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, SignerClientError>;
}

#[async_trait]
pub trait FinalizeBatchSigner: Send + Sync {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<SignedFinalizeBatch, SignerClientError>;
}

#[derive(Clone, Debug)]
pub struct RemoteCommitBatchSigner {
    endpoint: String,
    token: SecretString,
    client: reqwest::Client,
    validity_secs: u64,
    max_boc_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct RemoteFinalizeBatchSigner {
    endpoint: String,
    token: SecretString,
    client: reqwest::Client,
    validity_secs: u64,
    max_boc_bytes: usize,
}

impl RemoteCommitBatchSigner {
    pub fn from_config(config: &NodeConfig) -> Option<Self> {
        Some(Self::new(
            config.l1_commit_signer_endpoint.clone()?,
            config.l1_commit_signer_token.clone()?,
        ))
    }

    pub fn new(endpoint: String, token: SecretString) -> Self {
        Self {
            endpoint,
            token,
            client: reqwest::Client::new(),
            validity_secs: DEFAULT_SIGNER_VALIDITY_SECS,
            max_boc_bytes: DEFAULT_SIGNER_MAX_BODY_BYTES,
        }
    }
}

impl RemoteFinalizeBatchSigner {
    pub fn from_config(config: &NodeConfig) -> Option<Self> {
        Some(Self::new(
            config.l1_finalize_signer_endpoint.clone()?,
            config.l1_finalize_signer_token.clone()?,
        ))
    }

    pub fn new(endpoint: String, token: SecretString) -> Self {
        Self {
            endpoint,
            token,
            client: reqwest::Client::new(),
            validity_secs: DEFAULT_SIGNER_VALIDITY_SECS,
            max_boc_bytes: DEFAULT_SIGNER_MAX_BODY_BYTES,
        }
    }
}

#[async_trait]
impl CommitBatchSigner for RemoteCommitBatchSigner {
    async fn sign_commit_batch(
        &self,
        request: CommitBatchSignRequest,
    ) -> Result<SignedCommitBatch, SignerClientError> {
        let now = unix_time();
        let request_id = commit_request_id(&request);
        let envelope = TypedSignRequest::commit_batch(
            request_id.clone(),
            now.saturating_add(self.validity_secs),
            request,
        );
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.token.expose())
            .json(&envelope)
            .send()
            .await
            .map_err(SignerClientError::Http)?;
        if !response.status().is_success() {
            return Err(SignerClientError::Rejected("signer_rejected"));
        }
        let signed = response
            .json::<SignedExternalMessage>()
            .await
            .map_err(SignerClientError::Http)?;
        signed
            .into_commit_batch(&request_id, unix_time(), self.max_boc_bytes)
            .map_err(SignerClientError::Validation)
    }
}

#[async_trait]
impl FinalizeBatchSigner for RemoteFinalizeBatchSigner {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<SignedFinalizeBatch, SignerClientError> {
        let now = unix_time();
        let request_id = finalize_request_id(&request);
        let envelope = TypedSignRequest::finalize_batch(
            request_id.clone(),
            now.saturating_add(self.validity_secs),
            request,
        );
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.token.expose())
            .json(&envelope)
            .send()
            .await
            .map_err(SignerClientError::Http)?;
        if !response.status().is_success() {
            return Err(SignerClientError::Rejected("signer_rejected"));
        }
        let signed = response
            .json::<SignedExternalMessage>()
            .await
            .map_err(SignerClientError::Http)?;
        signed
            .into_finalize_batch(&request_id, unix_time(), self.max_boc_bytes)
            .map_err(SignerClientError::Validation)
    }
}

#[derive(Debug, Error)]
pub enum SignerClientError {
    #[error("signer http request failed")]
    Http(reqwest::Error),
    #[error("{0}")]
    Rejected(&'static str),
    #[error("{0}")]
    Validation(#[from] SignerValidationError),
}

impl SignerClientError {
    pub fn safe_code(&self) -> &'static str {
        match self {
            Self::Http(_) => "signer_http_failed",
            Self::Rejected(code) => code,
            Self::Validation(error) => error.safe_code(),
        }
    }
}
