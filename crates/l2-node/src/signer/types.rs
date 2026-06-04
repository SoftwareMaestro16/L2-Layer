use base64::prelude::{Engine as _, BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use l2_core::{Hash32, L2Block, L2BlockHeader};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use super::service::SignerConfigError;

pub const DEFAULT_SIGNER_VALIDITY_SECS: u64 = 300;
pub const DEFAULT_SIGNER_MAX_BODY_BYTES: usize = 16 * 1024;
pub const DEFAULT_SIGNER_RATE_LIMIT_PER_MINUTE: u32 = 60;
pub const DEFAULT_SIGNER_COMMAND_TIMEOUT_MS: u64 = 5_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchRootsA {
    pub prev_state_root: Hash32,
    pub state_root: Hash32,
    pub tx_root: Hash32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchRootsB {
    pub receipt_root: Hash32,
    pub withdrawal_root: Hash32,
    pub data_hash: Hash32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BatchCommitment {
    pub batch_no: u64,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub roots_a: BatchRootsA,
    pub roots_b: BatchRootsB,
}

impl BatchCommitment {
    pub fn from_block(block: &L2Block) -> Option<Self> {
        Self::from_header(&block.header)
    }

    pub fn from_header(header: &L2BlockHeader) -> Option<Self> {
        let batch_no = header.height.checked_add(1)?;
        Some(Self {
            batch_no,
            block_height: header.height,
            block_hash: header.block_hash(),
            roots_a: BatchRootsA {
                prev_state_root: header.prev_state_root,
                state_root: header.state_root,
                tx_root: header.tx_root,
            },
            roots_b: BatchRootsB {
                receipt_root: header.receipt_root,
                withdrawal_root: header.withdrawal_root,
                data_hash: header.data_hash,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitBatchSignRequest {
    pub rollup_root_address: String,
    pub sender_address: String,
    pub msg_value_nanoton: u64,
    pub commitment: BatchCommitment,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployContractSignRequest {
    pub sender_address: String,
    pub contract_name: String,
    pub state_init_boc_base64: String,
    pub init_body_boc_base64: Option<String>,
    pub msg_value_nanoton: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FinalizeBatchSignRequest {
    pub rollup_root_address: String,
    pub sender_address: String,
    pub batch_no: u64,
    pub msg_value_nanoton: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WithdrawalOperationSignRequest {
    pub rollup_root_address: String,
    pub sender_address: String,
    pub withdrawal_id: Hash32,
    pub msg_value_nanoton: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerRole {
    DeployerAdmin,
    Sequencer,
    VaultAdmin,
    Operator,
}

impl FromStr for SignerRole {
    type Err = SignerConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "deployer_admin" => Ok(Self::DeployerAdmin),
            "sequencer" => Ok(Self::Sequencer),
            "vault_admin" => Ok(Self::VaultAdmin),
            "operator" => Ok(Self::Operator),
            _ => Err(SignerConfigError::InvalidRole),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerAction {
    CommitBatch,
    DeployRollupRoot,
    DeployAssetVault,
    FinalizeBatch,
    ClaimWithdrawal,
    RetryWithdrawal,
    RetryRelease,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum TypedSignAction {
    CommitBatch(CommitBatchSignRequest),
    DeployRollupRoot(DeployContractSignRequest),
    DeployAssetVault(DeployContractSignRequest),
    FinalizeBatch(FinalizeBatchSignRequest),
    ClaimWithdrawal(WithdrawalOperationSignRequest),
    RetryWithdrawal(WithdrawalOperationSignRequest),
    RetryRelease(WithdrawalOperationSignRequest),
}

impl TypedSignAction {
    pub fn action(&self) -> SignerAction {
        match self {
            Self::CommitBatch(_) => SignerAction::CommitBatch,
            Self::DeployRollupRoot(_) => SignerAction::DeployRollupRoot,
            Self::DeployAssetVault(_) => SignerAction::DeployAssetVault,
            Self::FinalizeBatch(_) => SignerAction::FinalizeBatch,
            Self::ClaimWithdrawal(_) => SignerAction::ClaimWithdrawal,
            Self::RetryWithdrawal(_) => SignerAction::RetryWithdrawal,
            Self::RetryRelease(_) => SignerAction::RetryRelease,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypedSignRequest {
    pub request_id: String,
    pub role: SignerRole,
    pub valid_until: u64,
    #[serde(flatten)]
    pub action: TypedSignAction,
}

impl TypedSignRequest {
    pub fn commit_batch(
        request_id: String,
        valid_until: u64,
        payload: CommitBatchSignRequest,
    ) -> Self {
        Self {
            request_id,
            role: SignerRole::Sequencer,
            valid_until,
            action: TypedSignAction::CommitBatch(payload),
        }
    }

    pub fn validate(&self, now: u64) -> Result<(), SignerValidationError> {
        if self.request_id.trim().is_empty() || self.request_id.len() > 128 {
            return Err(SignerValidationError::BadRequestId);
        }
        if self.valid_until <= now {
            return Err(SignerValidationError::ExpiredRequest);
        }
        match &self.action {
            TypedSignAction::CommitBatch(request) => validate_commit_request(request),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedCommitBatch {
    pub boc_base64: String,
    pub signer_address: String,
    pub valid_until: u64,
}

impl SignedCommitBatch {
    pub fn validate(&self, now: u64, max_boc_bytes: usize) -> Result<(), SignerValidationError> {
        if self.signer_address.trim().is_empty() {
            return Err(SignerValidationError::MissingSignerAddress);
        }
        if self.valid_until <= now {
            return Err(SignerValidationError::ExpiredResponse);
        }
        validate_boc_base64(&self.boc_base64, max_boc_bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedExternalMessage {
    pub request_id: String,
    pub action: SignerAction,
    pub boc_base64: String,
    pub signer_address: String,
    pub valid_until: u64,
}

impl SignedExternalMessage {
    pub fn into_commit_batch(
        self,
        expected_request_id: &str,
        now: u64,
        max_boc_bytes: usize,
    ) -> Result<SignedCommitBatch, SignerValidationError> {
        if self.request_id != expected_request_id {
            return Err(SignerValidationError::RequestIdMismatch);
        }
        if self.action != SignerAction::CommitBatch {
            return Err(SignerValidationError::ActionMismatch);
        }
        let signed = SignedCommitBatch {
            boc_base64: self.boc_base64,
            signer_address: self.signer_address,
            valid_until: self.valid_until,
        };
        signed.validate(now, max_boc_bytes)?;
        Ok(signed)
    }
}

#[derive(Debug, Error)]
pub enum SignerValidationError {
    #[error("bad_request_id")]
    BadRequestId,
    #[error("expired_request")]
    ExpiredRequest,
    #[error("expired_response")]
    ExpiredResponse,
    #[error("missing_signer_address")]
    MissingSignerAddress,
    #[error("empty_boc")]
    EmptyBoc,
    #[error("malformed_boc")]
    MalformedBoc,
    #[error("oversized_boc")]
    OversizedBoc,
    #[error("request_id_mismatch")]
    RequestIdMismatch,
    #[error("action_mismatch")]
    ActionMismatch,
    #[error("invalid_commit_request")]
    InvalidCommitRequest,
}

impl SignerValidationError {
    pub fn safe_code(&self) -> &'static str {
        match self {
            Self::BadRequestId => "bad_request_id",
            Self::ExpiredRequest => "expired_request",
            Self::ExpiredResponse => "expired_response",
            Self::MissingSignerAddress => "missing_signer_address",
            Self::EmptyBoc => "empty_boc",
            Self::MalformedBoc => "malformed_boc",
            Self::OversizedBoc => "oversized_boc",
            Self::RequestIdMismatch => "request_id_mismatch",
            Self::ActionMismatch => "action_mismatch",
            Self::InvalidCommitRequest => "invalid_commit_request",
        }
    }
}

pub fn commit_request_id(request: &CommitBatchSignRequest) -> String {
    format!(
        "commit-batch-{}-{}",
        request.commitment.batch_no,
        request.commitment.block_hash.to_hex()
    )
}

pub fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn validate_commit_request(request: &CommitBatchSignRequest) -> Result<(), SignerValidationError> {
    if request.rollup_root_address.trim().is_empty()
        || request.sender_address.trim().is_empty()
        || request.msg_value_nanoton == 0
        || request.commitment.batch_no == 0
    {
        return Err(SignerValidationError::InvalidCommitRequest);
    }
    Ok(())
}

fn validate_boc_base64(value: &str, max_bytes: usize) -> Result<(), SignerValidationError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SignerValidationError::EmptyBoc);
    }
    let decoded = BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| SignerValidationError::MalformedBoc)?;
    if decoded.len() > max_bytes {
        return Err(SignerValidationError::OversizedBoc);
    }
    if !has_boc_magic(&decoded) {
        return Err(SignerValidationError::MalformedBoc);
    }
    Ok(())
}

fn has_boc_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xb5, 0xee, 0x9c, 0x72])
        || bytes.starts_with(&[0x68, 0xff, 0x65, 0xf3])
        || bytes.starts_with(&[0xac, 0xc3, 0xa7, 0x28])
}
