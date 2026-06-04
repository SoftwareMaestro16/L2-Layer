mod backend;
mod client;
mod service;
mod types;

pub use backend::{CommandSignerBackend, SignerBackendError, TypedSignerBackend};
pub use client::{CommitBatchSigner, RemoteCommitBatchSigner, SignerClientError};
pub use service::{build_signer_router, SignerConfigError, SignerServiceConfig};
pub use types::{
    unix_time, BatchCommitment, BatchRootsA, BatchRootsB, CommitBatchSignRequest,
    DeployContractSignRequest, FinalizeBatchSignRequest, SignedCommitBatch, SignedExternalMessage,
    SignerAction, SignerRole, SignerValidationError, TypedSignAction, TypedSignRequest,
    WithdrawalOperationSignRequest, DEFAULT_SIGNER_COMMAND_TIMEOUT_MS,
    DEFAULT_SIGNER_MAX_BODY_BYTES, DEFAULT_SIGNER_RATE_LIMIT_PER_MINUTE,
    DEFAULT_SIGNER_VALIDITY_SECS,
};

#[cfg(test)]
#[path = "../signer_tests.rs"]
mod tests;
