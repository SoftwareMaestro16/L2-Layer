use super::TvmGetMethodOutput;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::BagOfCells;

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
