//! C FFI ABI boundary and foreign language integration library for OpticsLab.
//!
//! Provides an `extern "C"` ABI boundary for integrating OpticsLab with C, C++,
//! Python (via `ctypes` or `cffi`), MATLAB, and Julia.

#![allow(unsafe_code)]
#![warn(missing_docs)]

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use optics_engine::{AnalysisRequest, EngineCommand, OpticalEngine};

// ---------------------------------------------------------------------------
// Error Codes & Thread-Local Storage
// ---------------------------------------------------------------------------

/// Status code indicating successful execution.
pub const OPTICS_OK: c_int = 0;
/// Status code indicating a null pointer was provided to an API function.
pub const OPTICS_ERR_NULL_PTR: c_int = -1;
/// Status code indicating invalid UTF-8 string encoding.
pub const OPTICS_ERR_INVALID_UTF8: c_int = -2;
/// Status code indicating JSON serialization or deserialization failure.
pub const OPTICS_ERR_JSON: c_int = -3;
/// Status code indicating an optical engine computation failure.
pub const OPTICS_ERR_ENGINE: c_int = -4;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(msg.into());
    });
}

fn to_c_string(s: impl Into<String>) -> *mut c_char {
    let raw = s.into();
    match CString::new(raw) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Lifecycle & Allocation
// ---------------------------------------------------------------------------

/// Creates a new optical engine instance initialized with the default Cooke Triplet F/4.5.
///
/// # Returns
/// An opaque pointer to [`OpticalEngine`], or null if initialization fails.
#[unsafe(no_mangle)]
pub extern "C" fn optics_engine_create() -> *mut OpticalEngine {
    match OpticalEngine::new() {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Creates a new optical engine from a `.zmx` formatted string.
///
/// # Safety
/// `zmx_content` must be a valid, null-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_create_with_zmx(zmx_content: *const c_char) -> *mut OpticalEngine {
    if zmx_content.is_null() {
        set_last_error("Null pointer provided for zmx_content");
        return std::ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(zmx_content) };
    let zmx_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8 in zmx_content: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut engine = match OpticalEngine::new() {
        Ok(e) => e,
        Err(e) => {
            set_last_error(e.to_string());
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = engine.load_zmx_str(zmx_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(engine))
}

/// Frees an [`OpticalEngine`] instance previously created with `optics_engine_create*`.
///
/// # Safety
/// `engine` must be a valid pointer obtained from `optics_engine_create*`, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_free(engine: *mut OpticalEngine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// Frees a string previously allocated by this library and returned to C.
///
/// # Safety
/// `s` must be a pointer returned from an `optics_*` function, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Retrieves the most recent error message on the current thread, or null if none.
///
/// The returned string must be freed with [`optics_string_free`].
#[unsafe(no_mangle)]
pub extern "C" fn optics_last_error_message() -> *mut c_char {
    LAST_ERROR.with(|cell| {
        if let Some(ref msg) = *cell.borrow() {
            to_c_string(msg)
        } else {
            std::ptr::null_mut()
        }
    })
}

// ---------------------------------------------------------------------------
// Engine Commands & State
// ---------------------------------------------------------------------------

/// Loads a `.zmx` formatted string into an existing engine instance.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`]. `zmx_content` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_load_zmx(engine: *mut OpticalEngine, zmx_content: *const c_char) -> c_int {
    if engine.is_null() || zmx_content.is_null() {
        set_last_error("Null pointer supplied to optics_engine_load_zmx");
        return OPTICS_ERR_NULL_PTR;
    }

    let c_str = unsafe { CStr::from_ptr(zmx_content) };
    let zmx_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {e}"));
            return OPTICS_ERR_INVALID_UTF8;
        }
    };

    let eng = unsafe { &mut *engine };
    match eng.load_zmx_str(zmx_str) {
        Ok(()) => OPTICS_OK,
        Err(e) => {
            set_last_error(e.to_string());
            OPTICS_ERR_ENGINE
        }
    }
}

/// Returns the current optical engine status serialized as a JSON string.
///
/// The caller is responsible for freeing the returned string using [`optics_string_free`].
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_get_status_json(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }

    let eng = unsafe { &*engine };
    let status = eng.status();
    match serde_json::to_string(&status) {
        Ok(json) => to_c_string(json),
        Err(e) => {
            set_last_error(format!("JSON serialization error: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Runs prescription diagnostics and returns a JSON array of diagnostic messages.
///
/// The caller is responsible for freeing the returned string using [`optics_string_free`].
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_validate_json(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }

    let eng = unsafe { &*engine };
    let diags = eng.validate();
    match serde_json::to_string(&diags) {
        Ok(json) => to_c_string(json),
        Err(e) => {
            set_last_error(format!("JSON serialization error: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Executes a JSON-serialized [`EngineCommand`] on the optical prescription.
///
/// Returns a JSON string `{"success": true, "message": "..."}` or `{"success": false, "error": "..."}`.
/// The returned string must be freed with [`optics_string_free`].
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`]. `command_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_execute_command_json(
    engine: *mut OpticalEngine,
    command_json: *const c_char,
) -> *mut c_char {
    if engine.is_null() || command_json.is_null() {
        set_last_error("Null pointer provided to optics_engine_execute_command_json");
        return to_c_string(r#"{"success":false,"error":"Null pointer provided"}"#);
    }

    let c_str = unsafe { CStr::from_ptr(command_json) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Invalid UTF-8: {e}");
            set_last_error(&msg);
            return to_c_string(format!(r#"{{"success":false,"error":"{msg}"}}"#));
        }
    };

    let cmd: EngineCommand = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Invalid command JSON: {e}");
            set_last_error(&msg);
            return to_c_string(format!(r#"{{"success":false,"error":"{msg}"}}"#));
        }
    };

    let eng = unsafe { &mut *engine };
    match eng.execute(cmd) {
        Ok(desc) => to_c_string(format!(
            r#"{{"success":true,"message":{},"revision":{}}}"#,
            serde_json::to_string(&desc).unwrap_or_default(),
            eng.revision()
        )),
        Err(e) => {
            let msg = e.to_string();
            set_last_error(&msg);
            to_c_string(format!(
                r#"{{"success":false,"error":{}}}"#,
                serde_json::to_string(&msg).unwrap_or_default()
            ))
        }
    }
}

/// Undoes the last executed command on the optical prescription.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_undo(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        return to_c_string(r#"{"success":false,"error":"Null engine pointer"}"#);
    }
    let eng = unsafe { &mut *engine };
    match eng.undo() {
        Ok(desc) => to_c_string(format!(
            r#"{{"success":true,"message":{},"revision":{}}}"#,
            serde_json::to_string(&desc).unwrap_or_default(),
            eng.revision()
        )),
        Err(e) => to_c_string(format!(
            r#"{{"success":false,"error":{}}}"#,
            serde_json::to_string(&e.to_string()).unwrap_or_default()
        )),
    }
}

/// Redoes the previously undone command on the optical prescription.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_redo(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        return to_c_string(r#"{"success":false,"error":"Null engine pointer"}"#);
    }
    let eng = unsafe { &mut *engine };
    match eng.redo() {
        Ok(desc) => to_c_string(format!(
            r#"{{"success":true,"message":{},"revision":{}}}"#,
            serde_json::to_string(&desc).unwrap_or_default(),
            eng.revision()
        )),
        Err(e) => to_c_string(format!(
            r#"{{"success":false,"error":{}}}"#,
            serde_json::to_string(&e.to_string()).unwrap_or_default()
        )),
    }
}

// ---------------------------------------------------------------------------
// Optical Analysis
// ---------------------------------------------------------------------------

/// Runs an optical analysis calculation from a JSON-serialized [`AnalysisRequest`].
///
/// Returns a JSON string containing the serialized [`optics_engine::AnalysisResult`].
/// The returned string must be freed with [`optics_string_free`].
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`]. `request_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_run_analysis_json(
    engine: *mut OpticalEngine,
    request_json: *const c_char,
) -> *mut c_char {
    if engine.is_null() || request_json.is_null() {
        set_last_error("Null pointer passed to optics_engine_run_analysis_json");
        return std::ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(request_json) };
    let json_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {e}"));
            return std::ptr::null_mut();
        }
    };

    let req: AnalysisRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("Invalid AnalysisRequest JSON: {e}"));
            return std::ptr::null_mut();
        }
    };

    let eng = unsafe { &mut *engine };
    match eng.run_analysis(&req) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(out_json) => to_c_string(out_json),
            Err(e) => {
                set_last_error(format!("Serialization error: {e}"));
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Export Routines
// ---------------------------------------------------------------------------

/// Exports the 2D layout as an SVG vector graphic string.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_export_svg(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }
    let eng = unsafe { &*engine };
    match eng.export_svg(None) {
        Ok(svg) => to_c_string(svg),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Exports the 2D/3D layout as an AutoCAD Release 12 DXF string.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_export_dxf(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }
    let eng = unsafe { &*engine };
    match eng.export_dxf(None) {
        Ok(dxf) => to_c_string(dxf),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Exports the optical prescription to a `.zmx` formatted string.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_export_zmx(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }
    let eng = unsafe { &*engine };
    match eng.export_zmx() {
        Ok(zmx) => to_c_string(zmx),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Exports ISO 10110 manufacturing drawing tables in Markdown format.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_export_iso10110_md(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }
    let eng = unsafe { &*engine };
    to_c_string(eng.export_iso10110_markdown())
}

/// Exports the native project workspace schema as a JSON string.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_export_project_json(engine: *mut OpticalEngine) -> *mut c_char {
    if engine.is_null() {
        set_last_error("Null engine pointer");
        return std::ptr::null_mut();
    }
    let eng = unsafe { &*engine };
    match eng.export_project_json() {
        Ok(json) => to_c_string(json),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Searches the glass database and returns matching entries as a JSON array.
///
/// # Safety
/// `engine` must point to an active [`OpticalEngine`]. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn optics_engine_search_glasses_json(
    engine: *mut OpticalEngine,
    query: *const c_char,
) -> *mut c_char {
    if engine.is_null() || query.is_null() {
        set_last_error("Null pointer provided to optics_engine_search_glasses_json");
        return std::ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(query) };
    let q_str = match c_str.to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Invalid UTF-8: {e}"));
            return std::ptr::null_mut();
        }
    };

    let eng = unsafe { &*engine };
    let matches = eng.search_glasses(q_str);
    match serde_json::to_string(&matches) {
        Ok(json) => to_c_string(json),
        Err(e) => {
            set_last_error(format!("JSON serialization error: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_c_ffi_lifecycle_and_analysis() -> TestResult {
        let engine = optics_engine_create();
        assert!(!engine.is_null());

        // Status JSON
        let status_ptr = unsafe { optics_engine_get_status_json(engine) };
        assert!(!status_ptr.is_null());
        let status_cstr = unsafe { CStr::from_ptr(status_ptr) };
        let status_str = status_cstr.to_str()?;
        assert!(status_str.contains("Cooke Triplet"));
        unsafe { optics_string_free(status_ptr) };

        // Paraxial Analysis
        let req_json = CString::new(r#"{"type":"Paraxial"}"#)?;
        let analysis_ptr = unsafe { optics_engine_run_analysis_json(engine, req_json.as_ptr()) };
        assert!(!analysis_ptr.is_null());
        let analysis_cstr = unsafe { CStr::from_ptr(analysis_ptr) };
        let analysis_str = analysis_cstr.to_str()?;
        assert!(analysis_str.contains("efl_mm"));
        unsafe { optics_string_free(analysis_ptr) };

        // Execute Command (Set EPD)
        let cmd_json = CString::new(r#"{"command":"SetEntrancePupilDiameter","args":{"epd_mm":15.0}}"#)?;
        let cmd_res_ptr = unsafe { optics_engine_execute_command_json(engine, cmd_json.as_ptr()) };
        assert!(!cmd_res_ptr.is_null());
        let cmd_res_cstr = unsafe { CStr::from_ptr(cmd_res_ptr) };
        let cmd_res_str = cmd_res_cstr.to_str()?;
        assert!(cmd_res_str.contains(r#""success":true"#));
        unsafe { optics_string_free(cmd_res_ptr) };

        // Undo
        let undo_ptr = unsafe { optics_engine_undo(engine) };
        assert!(!undo_ptr.is_null());
        let undo_cstr = unsafe { CStr::from_ptr(undo_ptr) };
        assert!(undo_cstr.to_str()?.contains(r#""success":true"#));
        unsafe { optics_string_free(undo_ptr) };

        // SVG Export
        let svg_ptr = unsafe { optics_engine_export_svg(engine) };
        assert!(!svg_ptr.is_null());
        let svg_cstr = unsafe { CStr::from_ptr(svg_ptr) };
        assert!(svg_cstr.to_str()?.contains("<svg"));
        unsafe { optics_string_free(svg_ptr) };

        // Free Engine
        unsafe { optics_engine_free(engine) };
        Ok(())
    }

    #[test]
    fn test_c_ffi_null_safety() {
        let res = unsafe { optics_engine_load_zmx(std::ptr::null_mut(), std::ptr::null()) };
        assert_eq!(res, OPTICS_ERR_NULL_PTR);

        let err_ptr = optics_last_error_message();
        assert!(!err_ptr.is_null());
        unsafe { optics_string_free(err_ptr) };
    }
}
