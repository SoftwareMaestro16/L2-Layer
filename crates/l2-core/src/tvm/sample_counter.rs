use super::{
    TvmAdapterError, TvmExecutionAdapter, TvmExecutionInput, TvmExecutionOutput,
    TvmExecutionStatus, TvmStateDelta,
};
use crate::crypto::Hash32;
use crate::enwallet::execute_enwallet_v5r1;
use crate::state::Account;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::{BagOfCells, CellBuilder};

pub const SAMPLE_COUNTER_INCREMENT_OPCODE: u32 = 0x534c_3201;
pub const SAMPLE_COUNTER_INCREMENT_GAS: u64 = 25;

const SAMPLE_COUNTER_CODE_MAGIC: u32 = 0x4c32_4343;
const SAMPLE_COUNTER_DATA_MAGIC: u32 = 0x4c32_4344;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleCounterContractState {
    pub code_hash: Hash32,
    pub data_hash: Hash32,
    pub storage_root: Hash32,
    pub code_boc_base64: String,
    pub data_boc_base64: String,
}

#[derive(Clone, Debug, Default)]
pub struct PrototypeTvmAdapter;

impl TvmExecutionAdapter for PrototypeTvmAdapter {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        if let Some(output) = execute_enwallet_v5r1(input)? {
            return Ok(output);
        }
        if input.contract_state.code_hash != sample_counter_code_hash() {
            return Err(TvmAdapterError::Unsupported);
        }
        Ok(execute_sample_counter(input))
    }
}

pub fn sample_counter_code_hash() -> Hash32 {
    cell_hash_from_base64(&sample_counter_code_boc_base64())
}

pub fn sample_counter_code_boc_base64() -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, SAMPLE_COUNTER_CODE_MAGIC)
        .expect("store sample counter code magic");
    cell_to_base64(builder)
}

pub fn sample_counter_initial_state(initial_counter: u64) -> SampleCounterContractState {
    let code_boc_base64 = sample_counter_code_boc_base64();
    let data_boc_base64 = sample_counter_data_boc_base64(initial_counter);
    let data_hash = cell_hash_from_base64(&data_boc_base64);
    SampleCounterContractState {
        code_hash: cell_hash_from_base64(&code_boc_base64),
        data_hash,
        storage_root: data_hash,
        code_boc_base64,
        data_boc_base64,
    }
}

pub fn sample_counter_data_hash(counter: u64) -> Hash32 {
    cell_hash_from_base64(&sample_counter_data_boc_base64(counter))
}

pub fn sample_counter_storage_root(counter: u64) -> Hash32 {
    sample_counter_data_hash(counter)
}

pub fn sample_counter_data_boc_base64(counter: u64) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, SAMPLE_COUNTER_DATA_MAGIC)
        .expect("store sample counter data magic")
        .store_u64(64, counter)
        .expect("store sample counter value");
    cell_to_base64(builder)
}

pub fn read_sample_counter_value(account: &Account) -> Result<u64, SampleCounterReadError> {
    if account.code_hash != sample_counter_code_hash() {
        return Err(SampleCounterReadError::UnsupportedCodeHash);
    }
    let data_boc_base64 = account
        .data_boc_base64
        .as_deref()
        .ok_or(SampleCounterReadError::MissingDataBoc)?;
    read_sample_counter_hashes(account.data_hash, account.storage_root, data_boc_base64)
}

fn read_sample_counter_hashes(
    data_hash: Hash32,
    storage_root: Hash32,
    data_boc_base64: &str,
) -> Result<u64, SampleCounterReadError> {
    let counter = decode_sample_counter_data_boc(data_boc_base64)?;
    let expected_hash = sample_counter_data_hash(counter);
    if data_hash != expected_hash || storage_root != expected_hash {
        return Err(SampleCounterReadError::DataHashMismatch);
    }
    Ok(counter)
}

fn execute_sample_counter(input: &TvmExecutionInput) -> TvmExecutionOutput {
    if input.gas_limit < SAMPLE_COUNTER_INCREMENT_GAS {
        return TvmExecutionOutput::rejected(input.gas_limit.max(1), "gas_exhausted");
    }
    let Some(data_boc_base64) = input.contract_state.data_boc_base64.as_deref() else {
        return TvmExecutionOutput::rejected(
            SAMPLE_COUNTER_INCREMENT_GAS,
            "sample_counter_bad_storage",
        );
    };
    let current = match read_sample_counter_hashes(
        input.contract_state.data_hash,
        input.contract_state.storage_root,
        data_boc_base64,
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
    let next_data_boc_base64 = sample_counter_data_boc_base64(next);
    let next_data_hash = sample_counter_data_hash(next);

    TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract: input.contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: Some(next_data_hash),
            data_boc_base64: Some(next_data_boc_base64),
            storage_root: Some(next_data_hash),
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

fn decode_sample_counter_data_boc(data_boc_base64: &str) -> Result<u64, SampleCounterReadError> {
    let boc = BASE64_STANDARD
        .decode(data_boc_base64.as_bytes())
        .map_err(|_| SampleCounterReadError::MalformedDataBoc)?;
    let root = BagOfCells::parse(&boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| SampleCounterReadError::MalformedDataBoc)?;
    let mut parser = root.parser();
    let magic = parser
        .load_u32(32)
        .map_err(|_| SampleCounterReadError::MalformedDataBoc)?;
    if magic != SAMPLE_COUNTER_DATA_MAGIC {
        return Err(SampleCounterReadError::MalformedDataBoc);
    }
    let counter = parser
        .load_u64(64)
        .map_err(|_| SampleCounterReadError::MalformedDataBoc)?;
    if parser.remaining_bits() != 0 || parser.remaining_refs() != 0 {
        return Err(SampleCounterReadError::MalformedDataBoc);
    }
    Ok(counter)
}

fn cell_hash_from_base64(value: &str) -> Hash32 {
    let boc = BASE64_STANDARD.decode(value.as_bytes()).expect("valid BoC");
    super::boc_single_root_hash(&boc).expect("valid single-root BoC")
}

fn cell_to_base64(mut builder: CellBuilder) -> String {
    let cell = builder.build().expect("build sample counter cell");
    let boc = BagOfCells::from_root(cell)
        .serialize(false)
        .expect("serialize sample counter cell");
    BASE64_STANDARD.encode(boc)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SampleCounterReadError {
    #[error("unsupported sample counter code hash")]
    UnsupportedCodeHash,
    #[error("malformed sample counter storage root")]
    MalformedStorageRoot,
    #[error("sample counter account has no data BoC")]
    MissingDataBoc,
    #[error("malformed sample counter data BoC")]
    MalformedDataBoc,
    #[error("sample counter data hash does not match storage root")]
    DataHashMismatch,
}
