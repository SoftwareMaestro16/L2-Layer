use async_trait::async_trait;
use serde_json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::types::{SignedCommitBatch, TypedSignRequest};

#[async_trait]
pub trait TypedSignerBackend: Clone + Send + Sync {
    async fn sign(
        &self,
        request: TypedSignRequest,
    ) -> Result<SignedCommitBatch, SignerBackendError>;
}

#[derive(Clone, Debug)]
pub struct CommandSignerBackend {
    command: PathBuf,
    timeout: Duration,
}

impl CommandSignerBackend {
    pub fn new(command: PathBuf, timeout: Duration) -> Self {
        Self { command, timeout }
    }
}

#[async_trait]
impl TypedSignerBackend for CommandSignerBackend {
    async fn sign(
        &self,
        request: TypedSignRequest,
    ) -> Result<SignedCommitBatch, SignerBackendError> {
        let input = serde_json::to_vec(&request)?;
        let mut child = Command::new(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(SignerBackendError::Failed("signer_stdin_unavailable"))?;
        stdin.write_all(&input).await?;
        drop(stdin);

        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| SignerBackendError::Timeout)??;
        if !output.status.success() {
            return Err(SignerBackendError::Failed("signer_command_failed"));
        }
        Ok(serde_json::from_slice::<SignedCommitBatch>(&output.stdout)?)
    }
}

#[derive(Debug, Error)]
pub enum SignerBackendError {
    #[error("{0}")]
    Failed(&'static str),
    #[error("signer backend io failed")]
    Io(#[from] std::io::Error),
    #[error("signer backend json failed")]
    Json(#[from] serde_json::Error),
    #[error("signer backend timed out")]
    Timeout,
}

impl SignerBackendError {
    pub(crate) fn safe_code(&self) -> &'static str {
        match self {
            Self::Failed(code) => code,
            Self::Io(_) => "signer_backend_io_failed",
            Self::Json(_) => "signer_backend_json_failed",
            Self::Timeout => "signer_backend_timeout",
        }
    }
}
