use crate::crypto::Hash32;
use crate::state::Account;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tonlib_core::cell::BagOfCells;

#[path = "tvm/cells.rs"]
mod cells;
pub mod emulator;
#[path = "tvm/sample_counter.rs"]
mod sample_counter;

pub use cells::{
    boc_single_root_hash, decode_contract_cell_boc_base64, ContractCell, ContractCellError,
    ContractCellField,
};
pub use emulator::{
    TvmEmulatorAdapter, TvmEmulatorBackend, TvmEmulatorBackendError, TvmEmulatorConfig,
    TvmEmulatorGetBackend, TvmEmulatorGetRequest, TvmEmulatorGetResult, TvmEmulatorRequest,
    TvmEmulatorResult,
};
pub use sample_counter::{
    read_sample_counter_value, sample_counter_code_boc_base64, sample_counter_code_hash,
    sample_counter_data_boc_base64, sample_counter_data_hash, sample_counter_initial_state,
    sample_counter_storage_root, PrototypeTvmAdapter, SampleCounterContractState,
    SampleCounterReadError, SAMPLE_COUNTER_INCREMENT_GAS, SAMPLE_COUNTER_INCREMENT_OPCODE,
};

pub use emulator::{RealTvmAdapter, TonlibTvmBackend};

pub const DEFAULT_MAX_TVM_BOC_BYTES: usize = 16 * 1024;
pub const DEFAULT_MAX_CONTRACT_CODE_BOC_BYTES: usize = DEFAULT_MAX_TVM_BOC_BYTES;
pub const DEFAULT_MAX_CONTRACT_DATA_BOC_BYTES: usize = DEFAULT_MAX_TVM_BOC_BYTES;
pub const DEFAULT_GETTER_GAS_LIMIT: u64 = 100_000;
pub const DEFAULT_MAX_GETTER_STACK_BOC_BYTES: usize = 16 * 1024;
const MAX_TVM_REASON_BYTES: usize = 64;

/// Deterministic context passed to the TON TVM adapter.
///
/// The adapter must not read wall-clock time, environment variables, network
/// state, or process-global mutable state. Every value needed for execution must
/// be passed through this context or through `TvmExecutionInput`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmExecutionContext {
    pub block_time: u64,
    pub block_height: u64,
    pub gas_coin_asset: u32,
    pub max_internal_messages: u32,
}

/// Complete deterministic input for one L2 contract call.
///
/// `contract` is the L2 account id of the called contract. `input_boc` is a
/// decoded, pre-validated, single-root TON BoC. The adapter receives an account
/// state snapshot and must return only an explicit `TvmExecutionOutput`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmExecutionInput {
    pub caller: Hash32,
    pub contract: Hash32,
    pub input_boc: Vec<u8>,
    pub gas_limit: u64,
    pub context: TvmExecutionContext,
    pub contract_state: TvmAccountState,
}

/// Deterministic input for a read-only contract get method.
///
/// Getters run off-chain against a snapshot of account code/data and must never
/// return a state delta. The stack BoC is the serialized TVM `VmStack` expected
/// by the underlying emulator. An empty vector represents no explicit stack
/// payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmGetMethodInput {
    pub contract: Hash32,
    pub method_id: i32,
    pub stack_boc: Vec<u8>,
    pub gas_limit: u64,
    pub context: TvmExecutionContext,
    pub contract_state: TvmAccountState,
}

/// Account snapshot visible to a TVM adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmAccountState {
    pub code_hash: Hash32,
    pub data_hash: Hash32,
    pub storage_root: Hash32,
    pub code_boc_base64: Option<String>,
    pub data_boc_base64: Option<String>,
    #[serde(with = "crate::types::serde_u128_string")]
    pub balance_nanoton: u128,
    pub last_lt: u64,
}

impl From<&Account> for TvmAccountState {
    fn from(account: &Account) -> Self {
        Self {
            code_hash: account.code_hash,
            data_hash: account.data_hash,
            storage_root: account.storage_root,
            code_boc_base64: account.code_boc_base64.clone(),
            data_boc_base64: account.data_boc_base64.clone(),
            balance_nanoton: account.balance(crate::types::L2_NATIVE_GAS_ASSET),
            last_lt: account.last_lt,
        }
    }
}

/// Target-contract-only mutation proposed by a TVM adapter.
///
/// The executor rejects deltas whose `contract` does not match the call target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmStateDelta {
    pub contract: Hash32,
    pub code_hash: Option<Hash32>,
    pub code_boc_base64: Option<String>,
    pub data_hash: Option<Hash32>,
    pub data_boc_base64: Option<String>,
    pub storage_root: Option<Hash32>,
}

/// Bounded async message emitted by contract execution.
///
/// The executor caps both message count and `body_boc` size before forwarding
/// messages to a future internal-message queue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmInternalMessage {
    pub from: Hash32,
    pub to: Hash32,
    #[serde(with = "crate::types::serde_u128_string")]
    pub value: u128,
    pub body_boc: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TvmExecutionStatus {
    Applied,
    Rejected { reason: String },
}

/// Result of deterministic TVM execution before executor-side validation.
///
/// `gas_used` must be in `1..=gas_limit`; rejected reasons must be stable
/// lowercase receipt codes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmExecutionOutput {
    pub status: TvmExecutionStatus,
    pub state_delta: Option<TvmStateDelta>,
    pub emitted_internal_messages: Vec<TvmInternalMessage>,
    pub gas_used: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmGetMethodOutput {
    pub vm_exit_code: i32,
    pub gas_used: u64,
    pub stack_boc_base64: String,
    pub missing_library: Option<String>,
}

impl TvmExecutionOutput {
    pub fn applied(gas_used: u64, state_delta: Option<TvmStateDelta>) -> Self {
        Self {
            status: TvmExecutionStatus::Applied,
            state_delta,
            emitted_internal_messages: vec![],
            gas_used,
        }
    }

    pub fn rejected(gas_used: u64, reason: impl Into<String>) -> Self {
        Self {
            status: TvmExecutionStatus::Rejected {
                reason: reason.into(),
            },
            state_delta: None,
            emitted_internal_messages: vec![],
            gas_used,
        }
    }
}

/// Adapter boundary for future TON TVM execution.
///
/// Implementations must be deterministic and must not read environment
/// variables, wall-clock time, network state, or persistent storage directly.
pub trait TvmExecutionAdapter {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError>;
}

pub trait TvmGetMethodAdapter {
    fn run_get_method(
        &self,
        input: &TvmGetMethodInput,
    ) -> Result<TvmGetMethodOutput, TvmAdapterError>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopTvmAdapter;

impl TvmExecutionAdapter for NoopTvmAdapter {
    fn execute(&self, _input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        Err(TvmAdapterError::Unsupported)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TvmAdapterError {
    #[error("tvm adapter is not implemented")]
    Unsupported,
    #[error("tvm adapter rejected execution: {reason}")]
    Rejected { reason: &'static str },
    #[error("tvm adapter execution failed: {reason}")]
    ExecutionFailed { reason: String },
}

impl TvmAdapterError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::Unsupported => "tvm_adapter_not_implemented",
            Self::Rejected { reason } => reason,
            Self::ExecutionFailed { .. } => "tvm_adapter_failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TvmBoundaryError {
    #[error("input BoC exceeds max size")]
    BocTooLarge,
    #[error("input BoC is malformed")]
    MalformedBoc,
    #[error("contract account is missing")]
    UnknownContract,
    #[error("contract account has no code")]
    ContractCodeMissing,
    #[error("adapter used zero gas")]
    ZeroGasUsed,
    #[error("adapter gas used exceeds gas limit")]
    GasUsedExceedsLimit,
    #[error("adapter emitted too many internal messages")]
    TooManyInternalMessages,
    #[error("adapter emitted an oversized internal message body")]
    InternalMessageBocTooLarge,
    #[error("adapter emitted an invalid receipt reason")]
    InvalidReceiptReason,
    #[error("adapter state delta targets another contract")]
    StateDeltaContractMismatch,
    #[error("adapter state delta code BoC hash mismatch")]
    StateDeltaCodeHashMismatch,
    #[error("adapter state delta data BoC hash mismatch")]
    StateDeltaDataHashMismatch,
    #[error("adapter state delta cell BoC is malformed")]
    StateDeltaMalformedCellBoc,
    #[error("adapter state delta cell BoC exceeds max size")]
    StateDeltaCellBocTooLarge,
}

impl TvmBoundaryError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::BocTooLarge => "boc_too_large",
            Self::MalformedBoc => "malformed_boc",
            Self::UnknownContract => "unknown_contract",
            Self::ContractCodeMissing => "contract_code_missing",
            Self::ZeroGasUsed => "tvm_zero_gas_used",
            Self::GasUsedExceedsLimit => "tvm_gas_used_exceeds_limit",
            Self::TooManyInternalMessages => "too_many_internal_messages",
            Self::InternalMessageBocTooLarge => "internal_message_boc_too_large",
            Self::InvalidReceiptReason => "invalid_tvm_receipt_reason",
            Self::StateDeltaContractMismatch => "tvm_state_delta_contract_mismatch",
            Self::StateDeltaCodeHashMismatch => "tvm_state_delta_code_hash_mismatch",
            Self::StateDeltaDataHashMismatch => "tvm_state_delta_data_hash_mismatch",
            Self::StateDeltaMalformedCellBoc => "tvm_state_delta_malformed_cell_boc",
            Self::StateDeltaCellBocTooLarge => "tvm_state_delta_cell_boc_too_large",
        }
    }
}

pub fn decode_call_body_boc_base64(
    body_boc_base64: &str,
    max_boc_bytes: usize,
) -> Result<Vec<u8>, TvmBoundaryError> {
    let max_encoded_bytes = max_boc_bytes
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4);
    if body_boc_base64.len() > max_encoded_bytes {
        return Err(TvmBoundaryError::BocTooLarge);
    }
    let input_boc = BASE64_STANDARD
        .decode(body_boc_base64.as_bytes())
        .map_err(|_| TvmBoundaryError::MalformedBoc)?;
    validate_call_body_boc(&input_boc, max_boc_bytes)?;
    Ok(input_boc)
}

pub fn validate_call_body_boc(
    input_boc: &[u8],
    max_boc_bytes: usize,
) -> Result<(), TvmBoundaryError> {
    if input_boc.is_empty() || input_boc.len() > max_boc_bytes {
        return Err(if input_boc.is_empty() {
            TvmBoundaryError::MalformedBoc
        } else {
            TvmBoundaryError::BocTooLarge
        });
    }
    BagOfCells::parse(input_boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| TvmBoundaryError::MalformedBoc)?;
    Ok(())
}

pub fn tvm_get_method_id(name: &str) -> Result<i32, TvmGetterInputError> {
    if !is_valid_get_method_name(name) {
        return Err(TvmGetterInputError::InvalidMethodName);
    }
    Ok(((u32::from(crc16_xmodem(name.as_bytes()))) | 0x10000) as i32)
}

pub fn decode_getter_stack_boc_base64(
    stack_boc_base64: Option<&str>,
    max_boc_bytes: usize,
) -> Result<Vec<u8>, TvmGetterInputError> {
    let Some(stack_boc_base64) = stack_boc_base64.filter(|value| !value.trim().is_empty()) else {
        return Ok(vec![]);
    };
    let max_encoded_bytes = max_boc_bytes
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4);
    if stack_boc_base64.len() > max_encoded_bytes {
        return Err(TvmGetterInputError::StackBocTooLarge);
    }
    let stack_boc = BASE64_STANDARD
        .decode(stack_boc_base64.as_bytes())
        .map_err(|_| TvmGetterInputError::MalformedStackBoc)?;
    if stack_boc.is_empty() || stack_boc.len() > max_boc_bytes {
        return Err(if stack_boc.is_empty() {
            TvmGetterInputError::MalformedStackBoc
        } else {
            TvmGetterInputError::StackBocTooLarge
        });
    }
    BagOfCells::parse(&stack_boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| TvmGetterInputError::MalformedStackBoc)?;
    Ok(stack_boc)
}

pub fn validate_tvm_get_method_output(
    output: &TvmGetMethodOutput,
    gas_limit: u64,
    max_stack_boc_bytes: usize,
) -> Result<(), TvmGetterOutputError> {
    if output.gas_used == 0 {
        return Err(TvmGetterOutputError::ZeroGasUsed);
    }
    if output.gas_used > gas_limit {
        return Err(TvmGetterOutputError::GasUsedExceedsLimit);
    }
    if output.stack_boc_base64.is_empty() {
        return Err(TvmGetterOutputError::MalformedStackBoc);
    }
    decode_getter_stack_boc_base64(Some(&output.stack_boc_base64), max_stack_boc_bytes).map_err(
        |error| match error {
            TvmGetterInputError::StackBocTooLarge => TvmGetterOutputError::StackBocTooLarge,
            _ => TvmGetterOutputError::MalformedStackBoc,
        },
    )?;
    Ok(())
}

pub fn validate_tvm_output(
    output: &TvmExecutionOutput,
    contract: Hash32,
    gas_limit: u64,
    max_internal_messages: u32,
    max_message_boc_bytes: usize,
) -> Result<(), TvmBoundaryError> {
    if output.gas_used == 0 {
        return Err(TvmBoundaryError::ZeroGasUsed);
    }
    if output.gas_used > gas_limit {
        return Err(TvmBoundaryError::GasUsedExceedsLimit);
    }
    if output.emitted_internal_messages.len() > max_internal_messages as usize {
        return Err(TvmBoundaryError::TooManyInternalMessages);
    }
    if output
        .emitted_internal_messages
        .iter()
        .any(|message| message.body_boc.len() > max_message_boc_bytes)
    {
        return Err(TvmBoundaryError::InternalMessageBocTooLarge);
    }
    if let Some(delta) = output.state_delta.as_ref() {
        if delta.contract != contract {
            return Err(TvmBoundaryError::StateDeltaContractMismatch);
        }
        validate_delta_cell(
            delta.code_boc_base64.as_deref(),
            delta.code_hash,
            max_message_boc_bytes,
            TvmBoundaryError::StateDeltaCodeHashMismatch,
        )?;
        validate_delta_cell(
            delta.data_boc_base64.as_deref(),
            delta.data_hash,
            max_message_boc_bytes,
            TvmBoundaryError::StateDeltaDataHashMismatch,
        )?;
    }
    if let TvmExecutionStatus::Rejected { reason } = &output.status {
        validate_receipt_reason(reason)?;
    }
    Ok(())
}

fn validate_delta_cell(
    boc_base64: Option<&str>,
    expected_hash: Option<Hash32>,
    max_boc_bytes: usize,
    mismatch: TvmBoundaryError,
) -> Result<(), TvmBoundaryError> {
    let (Some(boc_base64), Some(expected_hash)) = (boc_base64, expected_hash) else {
        return if boc_base64.is_none() && expected_hash.is_none() {
            Ok(())
        } else {
            Err(mismatch)
        };
    };
    let cell =
        decode_contract_cell_boc_base64(boc_base64, max_boc_bytes).map_err(
            |error| match error {
                ContractCellError::BocTooLarge => TvmBoundaryError::StateDeltaCellBocTooLarge,
                ContractCellError::MalformedBoc => TvmBoundaryError::StateDeltaMalformedCellBoc,
            },
        )?;
    if expected_hash != cell.cell_hash {
        return Err(mismatch);
    }
    Ok(())
}

fn validate_receipt_reason(reason: &str) -> Result<(), TvmBoundaryError> {
    if reason.is_empty()
        || reason.len() > MAX_TVM_REASON_BYTES
        || !reason
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_' || byte.is_ascii_digit())
    {
        return Err(TvmBoundaryError::InvalidReceiptReason);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TvmGetterInputError {
    #[error("invalid get method name")]
    InvalidMethodName,
    #[error("get method id must be positive")]
    InvalidMethodId,
    #[error("getter gas limit is invalid")]
    InvalidGasLimit,
    #[error("getter stack BoC exceeds max size")]
    StackBocTooLarge,
    #[error("getter stack BoC is malformed")]
    MalformedStackBoc,
}

impl TvmGetterInputError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::InvalidMethodName => "invalid_get_method_name",
            Self::InvalidMethodId => "invalid_get_method_id",
            Self::InvalidGasLimit => "invalid_getter_gas_limit",
            Self::StackBocTooLarge => "getter_stack_boc_too_large",
            Self::MalformedStackBoc => "malformed_getter_stack_boc",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TvmGetterOutputError {
    #[error("getter adapter used zero gas")]
    ZeroGasUsed,
    #[error("getter adapter gas used exceeds gas limit")]
    GasUsedExceedsLimit,
    #[error("getter stack BoC exceeds max size")]
    StackBocTooLarge,
    #[error("getter stack BoC is malformed")]
    MalformedStackBoc,
}

impl TvmGetterOutputError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::ZeroGasUsed => "tvm_getter_zero_gas_used",
            Self::GasUsedExceedsLimit => "tvm_getter_gas_used_exceeds_limit",
            Self::StackBocTooLarge => "tvm_getter_stack_boc_too_large",
            Self::MalformedStackBoc => "tvm_getter_stack_boc_malformed",
        }
    }
}

fn is_valid_get_method_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if name.len() > 64 || !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
