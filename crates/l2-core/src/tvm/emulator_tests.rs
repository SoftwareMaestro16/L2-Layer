use super::*;
use crate::crypto::sha256_bytes;
use crate::tvm::{
    sample_counter_initial_state, TvmAccountState, TvmExecutionContext, TvmExecutionInput,
    TvmExecutionStatus, TvmGetMethodInput,
};
use crate::types::L2_NATIVE_GAS_ASSET;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct RecordingBackend {
    requests: Arc<Mutex<Vec<TvmEmulatorRequest>>>,
    get_requests: Arc<Mutex<Vec<TvmEmulatorGetRequest>>>,
    result: TvmEmulatorResult,
    get_result: TvmEmulatorGetResult,
}

impl TvmEmulatorBackend for RecordingBackend {
    fn execute(
        &self,
        request: &TvmEmulatorRequest,
    ) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        Ok(self.result.clone())
    }
}

impl TvmEmulatorGetBackend for RecordingBackend {
    fn run_get_method(
        &self,
        request: &TvmEmulatorGetRequest,
    ) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError> {
        self.get_requests
            .lock()
            .expect("get requests lock")
            .push(request.clone());
        Ok(self.get_result.clone())
    }
}

fn sample_input() -> TvmExecutionInput {
    let sample = sample_counter_initial_state(5);
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

#[test]
fn emulator_request_and_output_replay_deterministically() {
    let input = sample_input();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let backend = RecordingBackend {
        requests: Arc::clone(&requests),
        get_requests: Arc::new(Mutex::new(Vec::new())),
        result: TvmEmulatorResult {
            accepted: true,
            vm_exit_code: 0,
            gas_used: 17,
            new_code_boc_base64: None,
            new_data_boc_base64: input.contract_state.data_boc_base64.clone(),
            actions_boc_base64: None,
            missing_library: None,
        },
        get_result: TvmEmulatorGetResult {
            vm_exit_code: 0,
            gas_used: 1,
            stack_boc_base64: BASE64_STANDARD.encode(empty_cell_boc()),
            missing_library: None,
        },
    };
    let adapter = TvmEmulatorAdapter::new(backend);

    let first = adapter.execute(&input).expect("first execution");
    let second = adapter.execute(&input).expect("second execution");

    assert_eq!(first, second);
    assert_eq!(first.status, TvmExecutionStatus::Applied);
    assert_eq!(first.gas_used, 17);
    let delta = first.state_delta.expect("state delta");
    assert_eq!(delta.contract, input.contract);
    assert_eq!(delta.data_hash, Some(input.contract_state.data_hash));

    let requests = requests.lock().expect("requests lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[0].gas_limit, input.gas_limit);
    assert_eq!(
        requests[0].address,
        format!("0:{}", input.contract.to_hex())
    );
    assert_eq!(requests[0].unixtime, input.context.block_time as u32);
    assert_eq!(requests[0].balance_nanoton, 123);
    assert_eq!(requests[0].rand_seed_hex.len(), 64);
}

#[test]
fn emulator_exit_code_maps_to_deterministic_rejection() {
    let input = sample_input();
    let backend = RecordingBackend {
        requests: Arc::new(Mutex::new(Vec::new())),
        get_requests: Arc::new(Mutex::new(Vec::new())),
        result: TvmEmulatorResult {
            accepted: true,
            vm_exit_code: 13,
            gas_used: 20,
            new_code_boc_base64: None,
            new_data_boc_base64: None,
            actions_boc_base64: None,
            missing_library: None,
        },
        get_result: TvmEmulatorGetResult {
            vm_exit_code: 0,
            gas_used: 1,
            stack_boc_base64: BASE64_STANDARD.encode(empty_cell_boc()),
            missing_library: None,
        },
    };
    let adapter = TvmEmulatorAdapter::new(backend);

    let output = adapter.execute(&input).expect("execution");

    assert_eq!(
        output.status,
        TvmExecutionStatus::Rejected {
            reason: "tvm_exit_code_13".to_owned()
        }
    );
    assert_eq!(output.gas_used, 20);
    assert_eq!(output.state_delta, None);
}

#[test]
fn emulator_get_method_request_replays_deterministically() {
    let input = sample_input();
    let get_input = TvmGetMethodInput {
        contract: input.contract,
        method_id: 0x12345,
        stack_boc: empty_cell_boc(),
        gas_limit: 99,
        context: input.context,
        contract_state: input.contract_state,
    };
    let get_requests = Arc::new(Mutex::new(Vec::new()));
    let stack_boc_base64 = BASE64_STANDARD.encode(empty_cell_boc());
    let backend = RecordingBackend {
        requests: Arc::new(Mutex::new(Vec::new())),
        get_requests: Arc::clone(&get_requests),
        result: TvmEmulatorResult {
            accepted: true,
            vm_exit_code: 0,
            gas_used: 1,
            new_code_boc_base64: None,
            new_data_boc_base64: None,
            actions_boc_base64: None,
            missing_library: None,
        },
        get_result: TvmEmulatorGetResult {
            vm_exit_code: 0,
            gas_used: 23,
            stack_boc_base64: stack_boc_base64.clone(),
            missing_library: None,
        },
    };
    let adapter = TvmEmulatorAdapter::new(backend);

    let first = adapter.run_get_method(&get_input).expect("first getter");
    let second = adapter.run_get_method(&get_input).expect("second getter");

    assert_eq!(first, second);
    assert_eq!(first.vm_exit_code, 0);
    assert_eq!(first.gas_used, 23);
    assert_eq!(first.stack_boc_base64, stack_boc_base64);
    let requests = get_requests.lock().expect("get requests lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[0].method_id, 0x12345);
    assert_eq!(requests[0].gas_limit, 99);
    assert_eq!(requests[0].rand_seed_hex.len(), 64);
}

#[test]
fn tonlib_backend_missing_library_is_stable_failure() {
    let backend = TonlibTvmBackend::default().with_library_path("__missing_tonlibjson__");
    let request = TvmEmulatorRequest {
        code_boc_base64: String::new(),
        data_boc_base64: String::new(),
        message_body_boc_base64: String::new(),
        gas_limit: 1,
        address: "0:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        unixtime: 0,
        balance_nanoton: 0,
        rand_seed_hex: String::new(),
        config_boc_base64: String::new(),
        libraries_boc_base64: None,
    };

    let error = backend.execute(&request).expect_err("missing library");

    assert_eq!(error.reason, "library_not_found");
}
