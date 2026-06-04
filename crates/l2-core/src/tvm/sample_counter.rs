use super::{
    TvmAdapterError, TvmExecutionAdapter, TvmExecutionInput, TvmExecutionOutput,
    TvmExecutionStatus, TvmStateDelta,
};
use crate::crypto::{hash_domain, Hash32};
use crate::state::Account;
use tonlib_core::cell::BagOfCells;

pub const SAMPLE_COUNTER_INCREMENT_OPCODE: u32 = 0x534c_3201;
pub const SAMPLE_COUNTER_INCREMENT_GAS: u64 = 25;

const SAMPLE_COUNTER_STORAGE_PREFIX: &[u8; 8] = b"L2CNTR01";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleCounterContractState {
    pub code_hash: Hash32,
    pub data_hash: Hash32,
    pub storage_root: Hash32,
}

#[derive(Clone, Debug, Default)]
pub struct PrototypeTvmAdapter;

impl TvmExecutionAdapter for PrototypeTvmAdapter {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        if input.contract_state.code_hash != sample_counter_code_hash() {
            return Err(TvmAdapterError::Unsupported);
        }
        Ok(execute_sample_counter(input))
    }
}

pub fn sample_counter_code_hash() -> Hash32 {
    hash_domain("l2.sample.counter.code.v1", &[])
}

pub fn sample_counter_initial_state(initial_counter: u64) -> SampleCounterContractState {
    SampleCounterContractState {
        code_hash: sample_counter_code_hash(),
        data_hash: sample_counter_data_hash(initial_counter),
        storage_root: sample_counter_storage_root(initial_counter),
    }
}

pub fn sample_counter_data_hash(counter: u64) -> Hash32 {
    hash_domain("l2.sample.counter.data.v1", &[&counter.to_be_bytes()])
}

pub fn sample_counter_storage_root(counter: u64) -> Hash32 {
    let mut bytes = [0u8; 32];
    bytes[..SAMPLE_COUNTER_STORAGE_PREFIX.len()].copy_from_slice(SAMPLE_COUNTER_STORAGE_PREFIX);
    bytes[8..16].copy_from_slice(&counter.to_be_bytes());
    Hash32::new(bytes)
}

pub fn read_sample_counter_value(account: &Account) -> Result<u64, SampleCounterReadError> {
    if account.code_hash != sample_counter_code_hash() {
        return Err(SampleCounterReadError::UnsupportedCodeHash);
    }
    read_sample_counter_hashes(account.data_hash, account.storage_root)
}

fn read_sample_counter_hashes(
    data_hash: Hash32,
    storage_root: Hash32,
) -> Result<u64, SampleCounterReadError> {
    let counter = decode_sample_counter_storage_root(storage_root)
        .ok_or(SampleCounterReadError::MalformedStorageRoot)?;
    if data_hash != sample_counter_data_hash(counter) {
        return Err(SampleCounterReadError::DataHashMismatch);
    }
    Ok(counter)
}

fn execute_sample_counter(input: &TvmExecutionInput) -> TvmExecutionOutput {
    if input.gas_limit < SAMPLE_COUNTER_INCREMENT_GAS {
        return TvmExecutionOutput::rejected(input.gas_limit.max(1), "gas_exhausted");
    }
    let current = match read_sample_counter_hashes(
        input.contract_state.data_hash,
        input.contract_state.storage_root,
    ) {
        Ok(value) => value,
        Err(_) => {
            return TvmExecutionOutput::rejected(
                SAMPLE_COUNTER_INCREMENT_GAS,
                "sample_counter_bad_storage",
            )
        }
    };
    let increment = match decode_increment_body(&input.input_boc) {
        Ok(value) => value,
        Err(reason) => return TvmExecutionOutput::rejected(SAMPLE_COUNTER_INCREMENT_GAS, reason),
    };
    let Some(next) = current.checked_add(u64::from(increment)) else {
        return TvmExecutionOutput::rejected(
            SAMPLE_COUNTER_INCREMENT_GAS,
            "sample_counter_overflow",
        );
    };

    TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract: input.contract,
            code_hash: None,
            data_hash: Some(sample_counter_data_hash(next)),
            storage_root: Some(sample_counter_storage_root(next)),
        }),
        emitted_internal_messages: vec![],
        gas_used: SAMPLE_COUNTER_INCREMENT_GAS,
    }
}

fn decode_increment_body(input_boc: &[u8]) -> Result<u32, &'static str> {
    let root = BagOfCells::parse(input_boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| "sample_counter_malformed_body")?;
    let mut parser = root.parser();
    let opcode = parser
        .load_u32(32)
        .map_err(|_| "sample_counter_malformed_body")?;
    if opcode != SAMPLE_COUNTER_INCREMENT_OPCODE {
        return Err("sample_counter_bad_opcode");
    }
    let increment = parser
        .load_u32(32)
        .map_err(|_| "sample_counter_malformed_body")?;
    if parser.remaining_bits() != 0 || parser.remaining_refs() != 0 {
        return Err("sample_counter_malformed_body");
    }
    if increment == 0 {
        return Err("sample_counter_bad_increment");
    }
    Ok(increment)
}

fn decode_sample_counter_storage_root(storage_root: Hash32) -> Option<u64> {
    let bytes = storage_root.as_bytes();
    if &bytes[..SAMPLE_COUNTER_STORAGE_PREFIX.len()] != SAMPLE_COUNTER_STORAGE_PREFIX {
        return None;
    }
    if bytes[16..].iter().any(|byte| *byte != 0) {
        return None;
    }
    Some(u64::from_be_bytes(bytes[8..16].try_into().ok()?))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SampleCounterReadError {
    #[error("unsupported sample counter code hash")]
    UnsupportedCodeHash,
    #[error("malformed sample counter storage root")]
    MalformedStorageRoot,
    #[error("sample counter data hash does not match storage root")]
    DataHashMismatch,
}
