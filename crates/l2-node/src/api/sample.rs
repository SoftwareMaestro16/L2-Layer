use super::{ApiError, AppState};
use axum::extract::{Path, State};
use axum::Json;
use l2_core::{
    decode_getter_stack_boc_base64, l2_raw_address, l2_user_friendly_address, parse_l2_address,
    read_enwallet_v5_state, read_sample_counter_value, tvm_get_method_id,
    validate_tvm_get_method_output, Account, Hash32, TvmAdapterError, TvmExecutionContext,
    TvmGetMethodAdapter, TvmGetMethodInput, TvmGetterInputError, ENWALLET_V5R1_INTERFACE,
    ENWALLET_V5R1_LABEL,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::task;
use tokio::time::timeout;

#[derive(Debug, Serialize)]
pub(super) struct SampleCounterResponse {
    pub(super) contract: Hash32,
    pub(super) contract_raw_address: String,
    pub(super) contract_friendly_address: String,
    pub(super) counter: u64,
    pub(super) code_hash: Hash32,
    pub(super) data_hash: Hash32,
    pub(super) storage_root: Hash32,
}

#[derive(Debug, Serialize)]
pub(super) struct ContractGetMethodResponse {
    pub(super) contract: Hash32,
    pub(super) contract_raw_address: String,
    pub(super) contract_friendly_address: String,
    pub(super) method: String,
    pub(super) method_id: i32,
    pub(super) gas_limit: u64,
    pub(super) gas_used: u64,
    pub(super) vm_exit_code: i32,
    pub(super) result: serde_json::Value,
    pub(super) source: &'static str,
    pub(super) read_only: bool,
    pub(super) state_root: Hash32,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContractGetMethodRequest {
    pub(super) method: String,
    #[serde(default)]
    pub(super) method_id: Option<i32>,
    #[serde(default)]
    pub(super) stack_boc_base64: Option<String>,
    #[serde(default)]
    pub(super) gas_limit: Option<u64>,
}

pub(super) async fn get_sample_counter(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SampleCounterResponse>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let sequencer = state.sequencer.read().await;
    let account = sequencer
        .state
        .account(id)
        .ok_or_else(|| ApiError::not_found("account not found"))?;
    let counter = read_sample_counter_value(account)
        .map_err(|_| ApiError::bad_request("not a sample counter contract"))?;
    Ok(Json(SampleCounterResponse {
        contract: id,
        contract_raw_address: l2_raw_address(id),
        contract_friendly_address: l2_user_friendly_address(id),
        counter,
        code_hash: account.code_hash,
        data_hash: account.data_hash,
        storage_root: account.storage_root,
    }))
}

pub(super) async fn get_contract_method(
    State(state): State<AppState>,
    Path((id, method)): Path<(String, String)>,
) -> Result<Json<ContractGetMethodResponse>, ApiError> {
    run_contract_get_method(
        state,
        id,
        ContractGetMethodRequest {
            method,
            method_id: None,
            stack_boc_base64: None,
            gas_limit: None,
        },
    )
    .await
}

pub(super) async fn post_contract_get_method(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ContractGetMethodRequest>,
) -> Result<Json<ContractGetMethodResponse>, ApiError> {
    run_contract_get_method(state, id, request).await
}

async fn run_contract_get_method(
    state: AppState,
    id: String,
    request: ContractGetMethodRequest,
) -> Result<Json<ContractGetMethodResponse>, ApiError> {
    let id = parse_l2_address(&id).map_err(|_| ApiError::bad_request("invalid account id"))?;
    let method_id = match request.method_id {
        Some(method_id) if method_id > 0 => method_id,
        Some(_) => {
            return Err(ApiError::bad_request(
                TvmGetterInputError::InvalidMethodId.rejection_reason(),
            ));
        }
        None => tvm_get_method_id(&request.method)
            .map_err(|error| ApiError::bad_request(error.rejection_reason()))?,
    };
    let gas_limit = validate_getter_gas_limit(&state, request.gas_limit)?;
    let stack_boc = decode_getter_stack_boc_base64(
        request.stack_boc_base64.as_deref(),
        state.tvm_getter_max_stack_boc_bytes,
    )
    .map_err(|error| ApiError::bad_request(error.rejection_reason()))?;
    let (account, state_root) = {
        let sequencer = state.sequencer.read().await;
        let account = sequencer
            .state
            .account(id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("account not found"))?;
        (account, sequencer.state.root_hash())
    };

    if let Some(result) = known_getter_result(&request.method, &account, &stack_boc)? {
        return Ok(Json(contract_get_method_response(
            id,
            request.method,
            method_id,
            gas_limit,
            0,
            0,
            result,
            "l2_state",
            state_root,
        )));
    }

    if state.tvm_adapter != l2_core::TvmAdapterMode::Real {
        return Err(ApiError::bad_request("get method not implemented"));
    }

    let output = run_real_getter(&state, id, account, method_id, stack_boc, gas_limit).await?;
    if let Some(_library) = output.missing_library.as_ref() {
        return Err(ApiError::bad_request("tvm_missing_library"));
    }
    validate_tvm_get_method_output(&output, gas_limit, state.tvm_getter_max_stack_boc_bytes)
        .map_err(|error| ApiError::bad_request(error.rejection_reason()))?;

    Ok(Json(contract_get_method_response(
        id,
        request.method,
        method_id,
        gas_limit,
        output.gas_used,
        output.vm_exit_code,
        serde_json::json!({
            "type": "vm_stack_boc",
            "stack_boc_base64": output.stack_boc_base64,
        }),
        "tvm_emulator",
        state_root,
    )))
}

fn known_getter_result(
    method: &str,
    account: &Account,
    stack_boc: &[u8],
) -> Result<Option<serde_json::Value>, ApiError> {
    if method == "currentCounter" || method == "counter" {
        reject_known_getter_args(stack_boc)?;
        let counter = read_sample_counter_value(account)
            .map_err(|_| ApiError::bad_request("not a sample counter contract"))?;
        return Ok(Some(serde_json::json!({
                "type": "uint64",
                "value": counter.to_string(),
        })));
    }

    if matches!(
        method,
        "seqno"
            | "get_wallet_id"
            | "get_subwallet_id"
            | "get_public_key"
            | "is_signature_allowed"
            | "get_extensions"
    ) {
        reject_known_getter_args(stack_boc)?;
        let wallet = read_enwallet_v5_state(account)
            .map_err(|_| ApiError::bad_request("not an EnWallet V5 R1 contract"))?;
        let result = match method {
            "seqno" => serde_json::json!({
                "type": "uint32",
                "value": wallet.seqno.to_string(),
            }),
            "get_wallet_id" | "get_subwallet_id" => serde_json::json!({
                "type": "uint32",
                "value": wallet.wallet_id.to_string(),
            }),
            "get_public_key" => serde_json::json!({
                "type": "uint256",
                "value": wallet.public_key.to_hex(),
            }),
            "is_signature_allowed" => serde_json::json!({
                "type": "bool",
                "value": wallet.is_signature_allowed,
            }),
            "get_extensions" => serde_json::json!({
                "type": "map<uint256,bool>",
                "count": wallet.extensions_count,
            }),
            _ => unreachable!(),
        };
        return Ok(Some(serde_json::json!({
                "interface": ENWALLET_V5R1_INTERFACE,
                "interface_label": ENWALLET_V5R1_LABEL,
                "result": result,
        })));
    }

    Ok(None)
}

fn reject_known_getter_args(stack_boc: &[u8]) -> Result<(), ApiError> {
    if stack_boc.is_empty() {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "getter parameters not supported for this method",
        ))
    }
}

async fn run_real_getter(
    state: &AppState,
    contract: Hash32,
    account: Account,
    method_id: i32,
    stack_boc: Vec<u8>,
    gas_limit: u64,
) -> Result<l2_core::TvmGetMethodOutput, ApiError> {
    let mut backend = l2_core::TonlibTvmBackend::default();
    if let Some(path) = state.tvm_tonlib_library_path.as_ref() {
        backend = backend.with_library_path(path.clone());
    }
    let adapter = l2_core::RealTvmAdapter::new(backend);
    let input = TvmGetMethodInput {
        contract,
        method_id,
        stack_boc,
        gas_limit,
        context: TvmExecutionContext {
            block_time: current_unix_time_u64(),
            block_height: account.last_lt,
            gas_coin_asset: l2_core::L2_NATIVE_GAS_ASSET,
            max_internal_messages: 0,
        },
        contract_state: (&account).into(),
    };
    let timeout_ms = state.tvm_getter_timeout_ms;
    let handle = task::spawn_blocking(move || adapter.run_get_method(&input));
    match timeout(Duration::from_millis(timeout_ms), handle).await {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(error))) => Err(tvm_adapter_api_error(error)),
        Ok(Err(error)) => {
            tracing::error!(?error, "tvm getter task failed");
            Err(ApiError::internal("tvm getter error"))
        }
        Err(_) => Err(ApiError::gateway_timeout("tvm getter timeout")),
    }
}

fn tvm_adapter_api_error(error: TvmAdapterError) -> ApiError {
    match error {
        TvmAdapterError::Rejected { reason } => ApiError::bad_request(reason),
        TvmAdapterError::Unsupported => ApiError::bad_request("tvm_adapter_not_implemented"),
        TvmAdapterError::ExecutionFailed { reason } => {
            tracing::warn!(reason, "tvm getter execution failed");
            ApiError::internal("tvm getter unavailable")
        }
    }
}

fn validate_getter_gas_limit(state: &AppState, gas_limit: Option<u64>) -> Result<u64, ApiError> {
    let gas_limit = gas_limit.unwrap_or(state.tvm_getter_default_gas_limit);
    if gas_limit == 0 || gas_limit > state.tvm_getter_max_gas_limit {
        return Err(ApiError::bad_request(
            TvmGetterInputError::InvalidGasLimit.rejection_reason(),
        ));
    }
    Ok(gas_limit)
}

fn contract_get_method_response(
    contract: Hash32,
    method: String,
    method_id: i32,
    gas_limit: u64,
    gas_used: u64,
    vm_exit_code: i32,
    result: serde_json::Value,
    source: &'static str,
    state_root: Hash32,
) -> ContractGetMethodResponse {
    ContractGetMethodResponse {
        contract,
        contract_raw_address: l2_raw_address(contract),
        contract_friendly_address: l2_user_friendly_address(contract),
        method,
        method_id,
        gas_limit,
        gas_used,
        vm_exit_code,
        result,
        source,
        read_only: true,
        state_root,
    }
}

fn current_unix_time_u64() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
