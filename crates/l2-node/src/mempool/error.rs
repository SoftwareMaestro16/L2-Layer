use l2_core::Hash32;
use serde_json::Error as SerdeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("wrong chain id")]
    WrongChainId,
    #[error("system deposit transactions are not accepted through the public mempool")]
    SystemTxNotAllowed,
    #[error("missing sender")]
    MissingSender,
    #[error("missing public key")]
    MissingPublicKey,
    #[error("missing signature")]
    MissingSignature,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("public key does not match sender")]
    PublicKeySenderMismatch,
    #[error("bad signature")]
    BadSignature,
    #[error("payload is {bytes} bytes, max is {max} bytes")]
    PayloadTooLarge { bytes: usize, max: usize },
    #[error("CallContract body_boc_base64 is {bytes} bytes, max is {max} bytes")]
    CallBodyTooLarge { bytes: usize, max: usize },
    #[error("CallContract body_boc_base64 is not valid standard base64")]
    BadCallBodyBase64,
    #[error("DeployContract {field} is {bytes} bytes, max is {max} bytes")]
    DeployBocTooLarge {
        field: &'static str,
        bytes: usize,
        max: usize,
    },
    #[error("DeployContract {field} is not valid standard base64")]
    BadDeployBocBase64 { field: &'static str },
    #[error("gas_limit {gas_limit} is outside [{min}, {max}]")]
    InvalidGasLimit { gas_limit: u64, min: u64, max: u64 },
    #[error("max_gas_price {gas_price} is below minimum {min}")]
    GasPriceTooLow { gas_price: u128, min: u128 },
    #[error("gas_limit * max_gas_price overflows")]
    TxFeeOverflow,
    #[error("max transaction fee {fee} is above limit {max}")]
    TxFeeTooHigh { fee: u128, max: u128 },
    #[error("duplicate transaction {0}")]
    DuplicateTx(Hash32),
    #[error("nonce {nonce} for account {account_id} is locked")]
    NonceLocked { account_id: Hash32, nonce: u64 },
    #[error("global mempool queue is full")]
    GlobalQueueFull,
    #[error("account {account_id} mempool queue is full")]
    AccountQueueFull { account_id: Hash32 },
    #[error("account {account_id} is rate limited")]
    RateLimited { account_id: Hash32 },
    #[error("mempool serialization failed: {0}")]
    Serialization(#[from] SerdeError),
    #[error("redis mempool failed: {0}")]
    Redis(#[from] redis::RedisError),
}

impl MempoolError {
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::DuplicateTx(_)
                | Self::NonceLocked { .. }
                | Self::GlobalQueueFull
                | Self::AccountQueueFull { .. }
                | Self::RateLimited { .. }
        )
    }

    pub(super) fn reason_code(&self) -> &'static str {
        match self {
            Self::WrongChainId => "wrong_chain_id",
            Self::SystemTxNotAllowed => "system_tx_not_allowed",
            Self::MissingSender => "missing_sender",
            Self::MissingPublicKey => "missing_public_key",
            Self::MissingSignature => "missing_signature",
            Self::InvalidPublicKey => "invalid_public_key",
            Self::PublicKeySenderMismatch => "public_key_sender_mismatch",
            Self::BadSignature => "bad_signature",
            Self::PayloadTooLarge { .. } => "payload_too_large",
            Self::CallBodyTooLarge { .. } => "call_body_too_large",
            Self::BadCallBodyBase64 => "bad_call_body_base64",
            Self::DeployBocTooLarge { .. } => "deploy_boc_too_large",
            Self::BadDeployBocBase64 { .. } => "bad_deploy_boc_base64",
            Self::InvalidGasLimit { .. } => "invalid_gas_limit",
            Self::GasPriceTooLow { .. } => "gas_price_too_low",
            Self::TxFeeOverflow => "tx_fee_overflow",
            Self::TxFeeTooHigh { .. } => "tx_fee_too_high",
            Self::DuplicateTx(_) => "duplicate_tx",
            Self::NonceLocked { .. } => "nonce_locked",
            Self::GlobalQueueFull => "global_queue_full",
            Self::AccountQueueFull { .. } => "account_queue_full",
            Self::RateLimited { .. } => "rate_limited",
            Self::Serialization(_) => "serialization",
            Self::Redis(_) => "redis",
        }
    }
}
