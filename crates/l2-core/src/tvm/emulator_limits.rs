use super::{TvmAdapterError, TvmEmulatorConfig};
use crate::tvm::{DEFAULT_MAX_GETTER_STACK_BOC_BYTES, DEFAULT_MAX_TVM_BOC_BYTES};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::BagOfCells;

pub(super) fn validate_emulator_config(config: &TvmEmulatorConfig) -> Result<(), TvmAdapterError> {
    validate_raw_boc(
        &config.config_boc,
        DEFAULT_MAX_TVM_BOC_BYTES,
        "tvm_config_boc_too_large",
        "tvm_config_boc_malformed",
    )?;
    if let Some(libraries) = config.libraries_boc.as_deref() {
        validate_raw_boc(
            libraries,
            DEFAULT_MAX_TVM_BOC_BYTES,
            "tvm_libraries_boc_too_large",
            "tvm_libraries_boc_malformed",
        )?;
    }
    Ok(())
}

pub(super) fn validate_getter_stack_input(stack_boc: &[u8]) -> Result<(), TvmAdapterError> {
    if stack_boc.is_empty() {
        return Ok(());
    }
    validate_raw_boc(
        stack_boc,
        DEFAULT_MAX_GETTER_STACK_BOC_BYTES,
        "tvm_getter_stack_boc_too_large",
        "tvm_getter_stack_boc_malformed",
    )
}

pub(super) fn validate_getter_stack_output(stack_boc_base64: &str) -> Result<(), TvmAdapterError> {
    validate_boc_base64(
        stack_boc_base64,
        DEFAULT_MAX_GETTER_STACK_BOC_BYTES,
        "tvm_getter_stack_boc_too_large",
        "tvm_getter_stack_boc_malformed",
    )
}

fn validate_boc_base64(
    value: &str,
    max_bytes: usize,
    too_large: &'static str,
    malformed: &'static str,
) -> Result<(), TvmAdapterError> {
    if value.len() > max_base64_len(max_bytes) {
        return Err(TvmAdapterError::Rejected { reason: too_large });
    }
    let bytes = BASE64_STANDARD
        .decode(value.as_bytes())
        .map_err(|_| TvmAdapterError::Rejected { reason: malformed })?;
    validate_raw_boc(&bytes, max_bytes, too_large, malformed)
}

fn validate_raw_boc(
    bytes: &[u8],
    max_bytes: usize,
    too_large: &'static str,
    malformed: &'static str,
) -> Result<(), TvmAdapterError> {
    if bytes.is_empty() {
        return Err(TvmAdapterError::Rejected { reason: malformed });
    }
    if bytes.len() > max_bytes {
        return Err(TvmAdapterError::Rejected { reason: too_large });
    }
    BagOfCells::parse(bytes)
        .and_then(BagOfCells::single_root)
        .map_err(|_| TvmAdapterError::Rejected { reason: malformed })?;
    Ok(())
}

fn max_base64_len(max_bytes: usize) -> usize {
    max_bytes
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4)
}
