use super::{TvmEmulatorBackend, TvmEmulatorBackendError, TvmEmulatorRequest, TvmEmulatorResult};
use std::ffi::{CStr, CString};

#[derive(Clone, Debug, Default)]
pub struct TonlibTvmBackend {
    pub vm_log_verbosity: u32,
}

impl TvmEmulatorBackend for TonlibTvmBackend {
    fn execute(
        &self,
        request: &TvmEmulatorRequest,
    ) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
        run_tonlib_emulator(self.vm_log_verbosity, request)
    }
}

fn run_tonlib_emulator(
    vm_log_verbosity: u32,
    request: &TvmEmulatorRequest,
) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
    let code = cstring(&request.code_boc_base64)?;
    let data = cstring(&request.data_boc_base64)?;
    let address = cstring(&request.address)?;
    let rand_seed = cstring(&request.rand_seed_hex)?;
    let config = cstring(&request.config_boc_base64)?;
    let body = cstring(&request.message_body_boc_base64)?;
    let libs = request
        .libraries_boc_base64
        .as_deref()
        .map(cstring)
        .transpose()?;

    unsafe {
        tonlib_sys::emulator_set_verbosity_level(0);
        let emulator =
            tonlib_sys::tvm_emulator_create(code.as_ptr(), data.as_ptr(), vm_log_verbosity);
        if emulator.is_null() {
            return Err(TvmEmulatorBackendError::new("create_failed"));
        }
        let _guard = EmulatorGuard(emulator);
        if let Some(libs) = libs.as_ref() {
            if !tonlib_sys::tvm_emulator_set_libraries(emulator, libs.as_ptr()) {
                return Err(TvmEmulatorBackendError::new("set_libraries_failed"));
            }
        }
        if !tonlib_sys::tvm_emulator_set_c7(
            emulator,
            address.as_ptr(),
            request.unixtime,
            request.balance_nanoton,
            rand_seed.as_ptr(),
            config.as_ptr(),
        ) {
            return Err(TvmEmulatorBackendError::new("set_c7_failed"));
        }
        if !tonlib_sys::tvm_emulator_set_gas_limit(emulator, request.gas_limit) {
            return Err(TvmEmulatorBackendError::new("set_gas_limit_failed"));
        }
        tonlib_sys::tvm_emulator_set_debug_enabled(emulator, 0);
        let raw = tonlib_sys::tvm_emulator_send_internal_message(emulator, body.as_ptr(), 0);
        if raw.is_null() {
            return Err(TvmEmulatorBackendError::new("empty_result"));
        }
        let json = CStr::from_ptr(raw)
            .to_str()
            .map_err(|_| TvmEmulatorBackendError::new("bad_result_utf8"))?;
        parse_tonlib_result(json)
    }
}

fn cstring(value: &str) -> Result<CString, TvmEmulatorBackendError> {
    CString::new(value).map_err(|_| TvmEmulatorBackendError::new("nul_byte"))
}

fn parse_tonlib_result(json: &str) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| TvmEmulatorBackendError::new("bad_json"))?;
    if !value
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Err(TvmEmulatorBackendError::new("emulator_rejected"));
    }
    Ok(TvmEmulatorResult {
        accepted: value
            .get("accepted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        vm_exit_code: value
            .get("vm_exit_code")
            .and_then(serde_json::Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(i32::MAX),
        gas_used: value
            .get("gas_used")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        new_code_boc_base64: string_field(&value, "new_code"),
        new_data_boc_base64: string_field(&value, "new_data"),
        actions_boc_base64: string_field(&value, "actions"),
        missing_library: string_field(&value, "missing_library"),
    })
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

struct EmulatorGuard(*mut std::os::raw::c_void);

impl Drop for EmulatorGuard {
    fn drop(&mut self) {
        unsafe {
            tonlib_sys::tvm_emulator_destroy(self.0);
        }
    }
}
