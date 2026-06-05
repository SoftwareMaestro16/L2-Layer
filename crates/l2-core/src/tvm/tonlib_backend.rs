use super::{
    TvmEmulatorBackend, TvmEmulatorBackendError, TvmEmulatorGetBackend, TvmEmulatorGetRequest,
    TvmEmulatorGetResult, TvmEmulatorRequest, TvmEmulatorResult,
};
use libloading::Library;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

type EmulatorSetVerbosityLevel = unsafe extern "C" fn(u32) -> bool;
type TvmEmulatorCreate = unsafe extern "C" fn(
    *const std::os::raw::c_char,
    *const std::os::raw::c_char,
    u32,
) -> *mut std::os::raw::c_void;
type TvmEmulatorSetLibraries =
    unsafe extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char) -> bool;
type TvmEmulatorSetC7 = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    *const std::os::raw::c_char,
    u32,
    u64,
    *const std::os::raw::c_char,
    *const std::os::raw::c_char,
) -> bool;
type TvmEmulatorSetGasLimit = unsafe extern "C" fn(*mut std::os::raw::c_void, u64) -> bool;
type TvmEmulatorSetDebugEnabled =
    unsafe extern "C" fn(*mut std::os::raw::c_void, std::os::raw::c_int) -> bool;
type TvmEmulatorSendInternalMessage = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    *const std::os::raw::c_char,
    u64,
) -> *const std::os::raw::c_char;
type TvmEmulatorRunGetMethod = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    std::os::raw::c_int,
    *const std::os::raw::c_char,
) -> *const std::os::raw::c_char;
type TvmEmulatorDestroy = unsafe extern "C" fn(*mut std::os::raw::c_void);

#[derive(Clone, Debug, Default)]
pub struct TonlibTvmBackend {
    pub vm_log_verbosity: u32,
    pub library_path: Option<PathBuf>,
}

impl TonlibTvmBackend {
    pub fn with_library_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.library_path = Some(path.into());
        self
    }
}

impl TvmEmulatorBackend for TonlibTvmBackend {
    fn execute(
        &self,
        request: &TvmEmulatorRequest,
    ) -> Result<TvmEmulatorResult, TvmEmulatorBackendError> {
        let bindings = TonlibBindings::load(self.library_path.as_deref())?;
        run_tonlib_emulator(&bindings, self.vm_log_verbosity, request)
    }
}

struct TonlibBindings {
    _library: Library,
    emulator_set_verbosity_level: EmulatorSetVerbosityLevel,
    tvm_emulator_create: TvmEmulatorCreate,
    tvm_emulator_set_libraries: TvmEmulatorSetLibraries,
    tvm_emulator_set_c7: TvmEmulatorSetC7,
    tvm_emulator_set_gas_limit: TvmEmulatorSetGasLimit,
    tvm_emulator_set_debug_enabled: TvmEmulatorSetDebugEnabled,
    tvm_emulator_send_internal_message: TvmEmulatorSendInternalMessage,
    tvm_emulator_run_get_method: TvmEmulatorRunGetMethod,
    tvm_emulator_destroy: TvmEmulatorDestroy,
}

impl TonlibBindings {
    fn load(path: Option<&Path>) -> Result<Arc<Self>, TvmEmulatorBackendError> {
        let library = match path {
            Some(path) => unsafe { Library::new(path) }
                .map_err(|_| TvmEmulatorBackendError::new("library_not_found"))?,
            None => load_default_library()?,
        };
        unsafe {
            Ok(Arc::new(Self {
                emulator_set_verbosity_level: *library
                    .get(b"emulator_set_verbosity_level\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_create: *library
                    .get(b"tvm_emulator_create\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_set_libraries: *library
                    .get(b"tvm_emulator_set_libraries\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_set_c7: *library
                    .get(b"tvm_emulator_set_c7\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_set_gas_limit: *library
                    .get(b"tvm_emulator_set_gas_limit\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_set_debug_enabled: *library
                    .get(b"tvm_emulator_set_debug_enabled\0")
                    .map_err(|_| {
                    TvmEmulatorBackendError::new("symbol_missing")
                })?,
                tvm_emulator_send_internal_message: *library
                    .get(b"tvm_emulator_send_internal_message\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_run_get_method: *library
                    .get(b"tvm_emulator_run_get_method\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                tvm_emulator_destroy: *library
                    .get(b"tvm_emulator_destroy\0")
                    .map_err(|_| TvmEmulatorBackendError::new("symbol_missing"))?,
                _library: library,
            }))
        }
    }
}

impl TvmEmulatorGetBackend for TonlibTvmBackend {
    fn run_get_method(
        &self,
        request: &TvmEmulatorGetRequest,
    ) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError> {
        let bindings = TonlibBindings::load(self.library_path.as_deref())?;
        run_tonlib_get_method(&bindings, self.vm_log_verbosity, request)
    }
}

fn load_default_library() -> Result<Library, TvmEmulatorBackendError> {
    for candidate in default_library_names() {
        if let Ok(library) = unsafe { Library::new(candidate) } {
            return Ok(library);
        }
    }
    Err(TvmEmulatorBackendError::new("library_not_found"))
}

#[cfg(target_os = "windows")]
fn default_library_names() -> &'static [&'static str] {
    &["tonlibjson.dll"]
}

#[cfg(target_os = "macos")]
fn default_library_names() -> &'static [&'static str] {
    &["libtonlibjson.dylib", "tonlibjson.dylib"]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_library_names() -> &'static [&'static str] {
    &["libtonlibjson.so", "tonlibjson.so"]
}

fn run_tonlib_emulator(
    bindings: &TonlibBindings,
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
        (bindings.emulator_set_verbosity_level)(0);
        let emulator =
            (bindings.tvm_emulator_create)(code.as_ptr(), data.as_ptr(), vm_log_verbosity);
        if emulator.is_null() {
            return Err(TvmEmulatorBackendError::new("create_failed"));
        }
        let _guard = EmulatorGuard {
            emulator,
            destroy: bindings.tvm_emulator_destroy,
        };
        if let Some(libs) = libs.as_ref() {
            if !(bindings.tvm_emulator_set_libraries)(emulator, libs.as_ptr()) {
                return Err(TvmEmulatorBackendError::new("set_libraries_failed"));
            }
        }
        if !(bindings.tvm_emulator_set_c7)(
            emulator,
            address.as_ptr(),
            request.unixtime,
            request.balance_nanoton,
            rand_seed.as_ptr(),
            config.as_ptr(),
        ) {
            return Err(TvmEmulatorBackendError::new("set_c7_failed"));
        }
        if !(bindings.tvm_emulator_set_gas_limit)(emulator, request.gas_limit) {
            return Err(TvmEmulatorBackendError::new("set_gas_limit_failed"));
        }
        (bindings.tvm_emulator_set_debug_enabled)(emulator, 0);
        let raw = (bindings.tvm_emulator_send_internal_message)(emulator, body.as_ptr(), 0);
        if raw.is_null() {
            return Err(TvmEmulatorBackendError::new("empty_result"));
        }
        let json = CStr::from_ptr(raw)
            .to_str()
            .map_err(|_| TvmEmulatorBackendError::new("bad_result_utf8"))?;
        parse_tonlib_result(json)
    }
}

fn run_tonlib_get_method(
    bindings: &TonlibBindings,
    vm_log_verbosity: u32,
    request: &TvmEmulatorGetRequest,
) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError> {
    let code = cstring(&request.code_boc_base64)?;
    let data = cstring(&request.data_boc_base64)?;
    let address = cstring(&request.address)?;
    let rand_seed = cstring(&request.rand_seed_hex)?;
    let config = cstring(&request.config_boc_base64)?;
    let stack = cstring(&request.stack_boc_base64)?;
    let libs = request
        .libraries_boc_base64
        .as_deref()
        .map(cstring)
        .transpose()?;

    unsafe {
        (bindings.emulator_set_verbosity_level)(0);
        let emulator =
            (bindings.tvm_emulator_create)(code.as_ptr(), data.as_ptr(), vm_log_verbosity);
        if emulator.is_null() {
            return Err(TvmEmulatorBackendError::new("create_failed"));
        }
        let _guard = EmulatorGuard {
            emulator,
            destroy: bindings.tvm_emulator_destroy,
        };
        if let Some(libs) = libs.as_ref() {
            if !(bindings.tvm_emulator_set_libraries)(emulator, libs.as_ptr()) {
                return Err(TvmEmulatorBackendError::new("set_libraries_failed"));
            }
        }
        if !(bindings.tvm_emulator_set_c7)(
            emulator,
            address.as_ptr(),
            request.unixtime,
            request.balance_nanoton,
            rand_seed.as_ptr(),
            config.as_ptr(),
        ) {
            return Err(TvmEmulatorBackendError::new("set_c7_failed"));
        }
        if !(bindings.tvm_emulator_set_gas_limit)(emulator, request.gas_limit) {
            return Err(TvmEmulatorBackendError::new("set_gas_limit_failed"));
        }
        (bindings.tvm_emulator_set_debug_enabled)(emulator, 0);
        let raw =
            (bindings.tvm_emulator_run_get_method)(emulator, request.method_id, stack.as_ptr());
        if raw.is_null() {
            return Err(TvmEmulatorBackendError::new("empty_result"));
        }
        let json = CStr::from_ptr(raw)
            .to_str()
            .map_err(|_| TvmEmulatorBackendError::new("bad_result_utf8"))?;
        parse_tonlib_get_result(json)
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

fn parse_tonlib_get_result(json: &str) -> Result<TvmEmulatorGetResult, TvmEmulatorBackendError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| TvmEmulatorBackendError::new("bad_json"))?;
    if !value
        .get("success")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Err(TvmEmulatorBackendError::new("emulator_rejected"));
    }
    Ok(TvmEmulatorGetResult {
        vm_exit_code: value
            .get("vm_exit_code")
            .and_then(serde_json::Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(i32::MAX),
        gas_used: value
            .get("gas_used")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        stack_boc_base64: string_field(&value, "stack").unwrap_or_default(),
        missing_library: string_field(&value, "missing_library"),
    })
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

struct EmulatorGuard {
    emulator: *mut std::os::raw::c_void,
    destroy: TvmEmulatorDestroy,
}

impl Drop for EmulatorGuard {
    fn drop(&mut self) {
        unsafe {
            (self.destroy)(self.emulator);
        }
    }
}
