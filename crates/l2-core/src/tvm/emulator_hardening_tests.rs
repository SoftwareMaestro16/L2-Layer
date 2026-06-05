use super::*;
use crate::crypto::sha256_bytes;
use crate::tvm::{sample_counter_initial_state, TvmAccountState, TvmExecutionContext};
use crate::types::L2_NATIVE_GAS_ASSET;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct HardeningBackend {
    requests: Arc<Mutex<usize>>,
    result: TvmEmulatorResult,
    get_result: TvmEmulatorGetResult,
}

impl TvmEmulatorBackend for HardeningBackend {
    fn execute(
        &self,
        _request: &TvmEmulatorRequest,
    ) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
        *self.requests.lock().expect("requests lock") += 1;
        Ok(self.result.clone())
    }
}

impl TvmEmulatorGetBackend for HardeningBackend {
    fn run_get_method(
        &self,
        _request: &TvmEmulatorGetRequest,
    ) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError> {
        *self.requests.lock().expect("requests lock") += 1;
        Ok(self.get_result.clone())
    }
}

#[test]
fn emulator_rejects_malformed_config_before_backend() {
    let requests = Arc::new(Mutex::new(0));
    let adapter =
        TvmEmulatorAdapter::new(backend(Arc::clone(&requests))).with_config(TvmEmulatorConfig {
            config_boc: vec![0, 1, 2],
            ..TvmEmulatorConfig::default()
        });

    let error = adapter.execute(&sample_input()).expect_err("bad config");

    assert_eq!(
        error,
        TvmAdapterError::Rejected {
            reason: "tvm_config_boc_malformed"
        }
    );
    assert_eq!(*requests.lock().expect("requests lock"), 0);
}

#[test]
fn emulator_rejects_oversized_actions_before_parsing() {
    let mut backend = backend(Arc::new(Mutex::new(0)));
    backend.result.actions_boc_base64 = Some(BASE64_STANDARD.encode(oversized_boc_bytes()));
    let adapter = TvmEmulatorAdapter::new(backend);

    let error = adapter
        .execute(&sample_input())
        .expect_err("oversized actions");

    assert_eq!(
        error,
        TvmAdapterError::Rejected {
            reason: "tvm_actions_boc_too_large"
        }
    );
}

#[test]
fn emulator_rejects_oversized_getter_stack_input_before_backend() {
    let requests = Arc::new(Mutex::new(0));
    let adapter = TvmEmulatorAdapter::new(backend(Arc::clone(&requests)));
    let input = TvmGetMethodInput {
        stack_boc: oversized_boc_bytes(),
        ..sample_get_input()
    };

    let error = adapter.run_get_method(&input).expect_err("oversized stack");

    assert_eq!(
        error,
        TvmAdapterError::Rejected {
            reason: "tvm_getter_stack_boc_too_large"
        }
    );
    assert_eq!(*requests.lock().expect("requests lock"), 0);
}

#[test]
fn emulator_rejects_oversized_getter_stack_output() {
    let mut backend = backend(Arc::new(Mutex::new(0)));
    backend.get_result.stack_boc_base64 = BASE64_STANDARD.encode(oversized_boc_bytes());
    let adapter = TvmEmulatorAdapter::new(backend);

    let error = adapter
        .run_get_method(&sample_get_input())
        .expect_err("oversized stack");

    assert_eq!(
        error,
        TvmAdapterError::Rejected {
            reason: "tvm_getter_stack_boc_too_large"
        }
    );
}

fn backend(requests: Arc<Mutex<usize>>) -> HardeningBackend {
    let input = sample_input();
    HardeningBackend {
        requests,
        result: TvmEmulatorResult {
            accepted: true,
            vm_exit_code: 0,
            gas_used: 10,
            new_code_boc_base64: None,
            new_data_boc_base64: input.contract_state.data_boc_base64.clone(),
            actions_boc_base64: None,
            missing_library: None,
        },
        get_result: TvmEmulatorGetResult {
            vm_exit_code: 0,
            gas_used: 10,
            stack_boc_base64: BASE64_STANDARD.encode(empty_cell_boc()),
            missing_library: None,
        },
    }
}

fn sample_input() -> TvmExecutionInput {
    let sample = sample_counter_initial_state(1);
    TvmExecutionInput {
        caller: sha256_bytes(b"caller"),
        contract: sha256_bytes(b"contract"),
        input_boc: empty_cell_boc(),
        gas_limit: 100,
        context: TvmExecutionContext {
            block_time: 42,
            block_height: 7,
            gas_coin_asset: L2_NATIVE_GAS_ASSET,
            max_internal_messages: 8,
        },
        contract_state: TvmAccountState {
            code_hash: sample.code_hash,
            data_hash: sample.data_hash,
            storage_root: sample.storage_root,
            code_boc_base64: Some(sample.code_boc_base64),
            data_boc_base64: Some(sample.data_boc_base64),
            balance_nanoton: 123,
            last_lt: 6,
        },
    }
}

fn sample_get_input() -> TvmGetMethodInput {
    let input = sample_input();
    TvmGetMethodInput {
        contract: input.contract,
        method_id: 0x12345,
        stack_boc: empty_cell_boc(),
        gas_limit: 100,
        context: input.context,
        contract_state: input.contract_state,
    }
}

fn oversized_boc_bytes() -> Vec<u8> {
    let mut bytes = empty_cell_boc();
    bytes.resize(DEFAULT_MAX_TVM_BOC_BYTES + 1, 0);
    bytes
}
