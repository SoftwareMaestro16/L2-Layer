use super::{
    decode_contract_cell_boc_base64, ContractCellError, TvmAdapterError, TvmExecutionAdapter,
    TvmExecutionInput, TvmExecutionOutput, TvmExecutionStatus, TvmGetMethodAdapter,
    TvmGetMethodInput, TvmGetMethodOutput, TvmStateDelta, DEFAULT_MAX_TVM_BOC_BYTES,
};
use crate::crypto::{hash_domain, Hash32};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tonlib_core::cell::{BagOfCells, CellBuilder};

#[path = "emulator_actions.rs"]
mod emulator_actions;
use emulator_actions::parse_actions;

const DEFAULT_TVM_WORKCHAIN: i32 = 0;
const EMPTY_CONFIG_CELL_ERROR: &str = "empty config cell must be serializable";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TvmEmulatorConfig {
    pub workchain: i32,
    pub config_boc: Vec<u8>,
    pub libraries_boc: Option<Vec<u8>>,
}

impl Default for TvmEmulatorConfig {
    fn default() -> Self {
        Self {
            workchain: DEFAULT_TVM_WORKCHAIN,
            config_boc: empty_cell_boc(),
            libraries_boc: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmEmulatorRequest {
    pub code_boc_base64: String,
    pub data_boc_base64: String,
    pub message_body_boc_base64: String,
    pub gas_limit: u64,
    pub address: String,
    pub unixtime: u32,
    pub balance_nanoton: u64,
    pub rand_seed_hex: String,
    pub config_boc_base64: String,
    pub libraries_boc_base64: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmEmulatorResult {
    pub accepted: bool,
    pub vm_exit_code: i32,
    pub gas_used: u64,
    pub new_code_boc_base64: Option<String>,
    pub new_data_boc_base64: Option<String>,
    pub actions_boc_base64: Option<String>,
    pub missing_library: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmEmulatorGetRequest {
    pub code_boc_base64: String,
    pub data_boc_base64: String,
    pub method_id: i32,
    pub stack_boc_base64: String,
    pub gas_limit: u64,
    pub address: String,
    pub unixtime: u32,
    pub balance_nanoton: u64,
    pub rand_seed_hex: String,
    pub config_boc_base64: String,
    pub libraries_boc_base64: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TvmEmulatorGetResult {
    pub vm_exit_code: i32,
    pub gas_used: u64,
    pub stack_boc_base64: String,
    pub missing_library: Option<String>,
}

pub trait TvmEmulatorBackend {
    fn execute(
        &self,
        request: &TvmEmulatorRequest,
    ) -> Result<TvmEmulatorResult, TvmEmulatorBackendError>;
}

pub trait TvmEmulatorGetBackend {
    fn run_get_method(
        &self,
        request: &TvmEmulatorGetRequest,
    ) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("tvm emulator backend failed: {reason}")]
pub struct TvmEmulatorBackendError {
    pub reason: String,
}

impl TvmEmulatorBackendError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TvmEmulatorAdapter<B> {
    backend: B,
    config: TvmEmulatorConfig,
}

impl<B> TvmEmulatorAdapter<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            config: TvmEmulatorConfig::default(),
        }
    }

    pub fn with_config(mut self, config: TvmEmulatorConfig) -> Self {
        self.config = config;
        self
    }
}

impl<B: TvmEmulatorBackend> TvmExecutionAdapter for TvmEmulatorAdapter<B> {
    fn execute(&self, input: &TvmExecutionInput) -> Result<TvmExecutionOutput, TvmAdapterError> {
        let image = self.load_image(input)?;
        let request = self.request_for(input, &image)?;
        let result =
            self.backend
                .execute(&request)
                .map_err(|error| TvmAdapterError::ExecutionFailed {
                    reason: error.reason,
                })?;
        self.output_for(input, image, result)
    }
}

impl<B: TvmEmulatorGetBackend> TvmGetMethodAdapter for TvmEmulatorAdapter<B> {
    fn run_get_method(
        &self,
        input: &TvmGetMethodInput,
    ) -> Result<TvmGetMethodOutput, TvmAdapterError> {
        let image = self.load_image_from_state(&input.contract_state)?;
        let request = self.get_request_for(input, &image)?;
        let result = self.backend.run_get_method(&request).map_err(|error| {
            TvmAdapterError::ExecutionFailed {
                reason: error.reason,
            }
        })?;
        Ok(TvmGetMethodOutput {
            vm_exit_code: result.vm_exit_code,
            gas_used: result.gas_used,
            stack_boc_base64: result.stack_boc_base64,
            missing_library: result.missing_library,
        })
    }
}

struct AccountImage {
    code_boc_base64: String,
    data_boc_base64: String,
    code_hash: Hash32,
}

impl<B> TvmEmulatorAdapter<B> {
    fn load_image(&self, input: &TvmExecutionInput) -> Result<AccountImage, TvmAdapterError> {
        self.load_image_from_state(&input.contract_state)
    }

    fn load_image_from_state(
        &self,
        contract_state: &super::TvmAccountState,
    ) -> Result<AccountImage, TvmAdapterError> {
        if contract_state.data_hash != contract_state.storage_root {
            return Err(TvmAdapterError::Rejected {
                reason: "tvm_contract_state_hash_mismatch",
            });
        }
        let code_boc =
            contract_state
                .code_boc_base64
                .as_deref()
                .ok_or(TvmAdapterError::Rejected {
                    reason: "tvm_contract_code_missing",
                })?;
        let data_boc =
            contract_state
                .data_boc_base64
                .as_deref()
                .ok_or(TvmAdapterError::Rejected {
                    reason: "tvm_contract_data_missing",
                })?;
        let code = decode_contract_cell_boc_base64(code_boc, DEFAULT_MAX_TVM_BOC_BYTES)
            .map_err(contract_cell_rejection)?;
        let data = decode_contract_cell_boc_base64(data_boc, DEFAULT_MAX_TVM_BOC_BYTES)
            .map_err(contract_cell_rejection)?;
        if code.cell_hash != contract_state.code_hash {
            return Err(TvmAdapterError::Rejected {
                reason: "tvm_contract_code_hash_mismatch",
            });
        }
        if data.cell_hash != contract_state.data_hash {
            return Err(TvmAdapterError::Rejected {
                reason: "tvm_contract_data_hash_mismatch",
            });
        }
        Ok(AccountImage {
            code_boc_base64: code.boc_base64,
            data_boc_base64: data.boc_base64,
            code_hash: code.cell_hash,
        })
    }

    fn request_for(
        &self,
        input: &TvmExecutionInput,
        image: &AccountImage,
    ) -> Result<TvmEmulatorRequest, TvmAdapterError> {
        let unixtime =
            u32::try_from(input.context.block_time).map_err(|_| TvmAdapterError::Rejected {
                reason: "tvm_invalid_context",
            })?;
        let balance_nanoton =
            u64::try_from(input.contract_state.balance_nanoton).map_err(|_| {
                TvmAdapterError::Rejected {
                    reason: "tvm_balance_overflow",
                }
            })?;
        Ok(TvmEmulatorRequest {
            code_boc_base64: image.code_boc_base64.clone(),
            data_boc_base64: image.data_boc_base64.clone(),
            message_body_boc_base64: BASE64_STANDARD.encode(&input.input_boc),
            gas_limit: input.gas_limit,
            address: raw_ton_address(self.config.workchain, input.contract),
            unixtime,
            balance_nanoton,
            rand_seed_hex: deterministic_rand_seed(input).to_hex(),
            config_boc_base64: BASE64_STANDARD.encode(&self.config.config_boc),
            libraries_boc_base64: self
                .config
                .libraries_boc
                .as_ref()
                .map(|boc| BASE64_STANDARD.encode(boc)),
        })
    }

    fn get_request_for(
        &self,
        input: &TvmGetMethodInput,
        image: &AccountImage,
    ) -> Result<TvmEmulatorGetRequest, TvmAdapterError> {
        let unixtime =
            u32::try_from(input.context.block_time).map_err(|_| TvmAdapterError::Rejected {
                reason: "tvm_invalid_context",
            })?;
        let balance_nanoton =
            u64::try_from(input.contract_state.balance_nanoton).map_err(|_| {
                TvmAdapterError::Rejected {
                    reason: "tvm_balance_overflow",
                }
            })?;
        Ok(TvmEmulatorGetRequest {
            code_boc_base64: image.code_boc_base64.clone(),
            data_boc_base64: image.data_boc_base64.clone(),
            method_id: input.method_id,
            stack_boc_base64: if input.stack_boc.is_empty() {
                String::new()
            } else {
                BASE64_STANDARD.encode(&input.stack_boc)
            },
            gas_limit: input.gas_limit,
            address: raw_ton_address(self.config.workchain, input.contract),
            unixtime,
            balance_nanoton,
            rand_seed_hex: deterministic_getter_rand_seed(input).to_hex(),
            config_boc_base64: BASE64_STANDARD.encode(&self.config.config_boc),
            libraries_boc_base64: self
                .config
                .libraries_boc
                .as_ref()
                .map(|boc| BASE64_STANDARD.encode(boc)),
        })
    }

    fn output_for(
        &self,
        input: &TvmExecutionInput,
        image: AccountImage,
        result: TvmEmulatorResult,
    ) -> Result<TvmExecutionOutput, TvmAdapterError> {
        if result.missing_library.is_some() {
            return Ok(TvmExecutionOutput::rejected(
                result.gas_used,
                "tvm_missing_library",
            ));
        }
        if !result.accepted {
            return Ok(TvmExecutionOutput::rejected(
                result.gas_used,
                "tvm_message_not_accepted",
            ));
        }
        if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
            return Ok(TvmExecutionOutput::rejected(
                result.gas_used,
                exit_code_reason(result.vm_exit_code),
            ));
        }

        let new_data_boc_base64 =
            result
                .new_data_boc_base64
                .as_deref()
                .ok_or(TvmAdapterError::Rejected {
                    reason: "tvm_missing_new_data",
                })?;
        let new_data =
            decode_contract_cell_boc_base64(new_data_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
                .map_err(contract_cell_rejection)?;

        let (code_boc_base64, code_hash) = match result.new_code_boc_base64.as_deref() {
            Some(value) => {
                let new_code = decode_contract_cell_boc_base64(value, DEFAULT_MAX_TVM_BOC_BYTES)
                    .map_err(contract_cell_rejection)?;
                if new_code.cell_hash == image.code_hash {
                    (None, None)
                } else {
                    (Some(new_code.boc_base64), Some(new_code.cell_hash))
                }
            }
            None => (None, None),
        };
        let emitted_internal_messages = parse_actions(
            result.actions_boc_base64.as_deref(),
            input.contract,
            self.config.workchain,
            input.context.max_internal_messages,
        )?;

        Ok(TvmExecutionOutput {
            status: TvmExecutionStatus::Applied,
            state_delta: Some(TvmStateDelta {
                contract: input.contract,
                code_hash,
                code_boc_base64,
                data_hash: Some(new_data.cell_hash),
                data_boc_base64: Some(new_data.boc_base64),
                storage_root: Some(new_data.cell_hash),
            }),
            emitted_internal_messages,
            gas_used: result.gas_used,
        })
    }
}

fn contract_cell_rejection(error: ContractCellError) -> TvmAdapterError {
    TvmAdapterError::Rejected {
        reason: match error {
            ContractCellError::BocTooLarge => "tvm_contract_boc_too_large",
            ContractCellError::MalformedBoc => "tvm_contract_boc_malformed",
        },
    }
}

fn deterministic_rand_seed(input: &TvmExecutionInput) -> Hash32 {
    let block_height = input.context.block_height.to_be_bytes();
    let last_lt = input.contract_state.last_lt.to_be_bytes();
    hash_domain(
        "entropis:tvm-rand-seed:v1",
        &[
            input.caller.as_bytes(),
            input.contract.as_bytes(),
            &block_height,
            &last_lt,
            input.contract_state.code_hash.as_bytes(),
            input.contract_state.data_hash.as_bytes(),
            &input.input_boc,
        ],
    )
}

fn deterministic_getter_rand_seed(input: &TvmGetMethodInput) -> Hash32 {
    let block_height = input.context.block_height.to_be_bytes();
    let last_lt = input.contract_state.last_lt.to_be_bytes();
    let method_id = input.method_id.to_be_bytes();
    hash_domain(
        "entropis:tvm-getter-rand-seed:v1",
        &[
            input.contract.as_bytes(),
            &method_id,
            &block_height,
            &last_lt,
            input.contract_state.code_hash.as_bytes(),
            input.contract_state.data_hash.as_bytes(),
            &input.stack_boc,
        ],
    )
}

fn raw_ton_address(workchain: i32, address: Hash32) -> String {
    format!("{workchain}:{}", address.to_hex())
}

fn exit_code_reason(exit_code: i32) -> String {
    if exit_code < 0 {
        format!("tvm_exit_code_neg{}", exit_code.unsigned_abs())
    } else {
        format!("tvm_exit_code_{exit_code}")
    }
}

fn empty_cell_boc() -> Vec<u8> {
    CellBuilder::new()
        .build()
        .and_then(|cell| BagOfCells::from_root(cell).serialize(false))
        .expect(EMPTY_CONFIG_CELL_ERROR)
}

#[cfg(test)]
#[path = "emulator_tests.rs"]
mod tests;

#[path = "tonlib_backend.rs"]
mod tonlib_backend;

pub use tonlib_backend::TonlibTvmBackend;

pub type RealTvmAdapter = TvmEmulatorAdapter<TonlibTvmBackend>;
