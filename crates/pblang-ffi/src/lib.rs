use std::ffi::{c_char, CStr, CString};
use std::ptr;

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

/// Opaque VM wrapper type used by the C FFI. Consumers reference this only as a pointer.
pub struct VmWrapper {
    vm: VM,
    last_error: CString,
}

fn set_error(w: &mut VmWrapper, msg: impl Into<String>) {
    w.last_error = CString::new(msg.into()).unwrap_or_else(|_| CString::new("error").unwrap());
}

fn clear_error(w: &mut VmWrapper) {
    w.last_error = CString::new("").unwrap();
}

fn escape_pblang_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('"');
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_to_pblang_expr(v: &serde_json::Value) -> Result<String, String> {
    match v {
        serde_json::Value::Null => Ok("nil".to_string()),
        serde_json::Value::Bool(b) => Ok(if *b {
            "true".to_string()
        } else {
            "false".to_string()
        }),
        serde_json::Value::Number(n) => {
            let s = n.to_string();
            if s.eq_ignore_ascii_case("nan")
                || s.eq_ignore_ascii_case("inf")
                || s.eq_ignore_ascii_case("-inf")
            {
                return Err("non-finite numbers are not supported in args_json".to_string());
            }
            Ok(s)
        }
        serde_json::Value::String(s) => Ok(escape_pblang_string(s)),
        serde_json::Value::Array(items) => {
            let mut out = String::from("[");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&json_to_pblang_expr(item)?);
            }
            out.push(']');
            Ok(out)
        }
        serde_json::Value::Object(map) => {
            let mut out = String::from("map{");
            let mut first = true;
            for (k, val) in map {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&escape_pblang_string(k));
                out.push_str(": ");
                out.push_str(&json_to_pblang_expr(val)?);
            }
            out.push('}');
            Ok(out)
        }
    }
}

/// Create a new pblang VM.
#[no_mangle]
pub extern "C" fn pblang_create_vm() -> *mut VmWrapper {
    let w = VmWrapper {
        vm: VM::with_stdlib(StdlibConfig::default()),
        last_error: CString::new("").unwrap(),
    };
    Box::into_raw(Box::new(w))
}

/// Create a new pblang VM with explicit capability sandbox.
/// Pass 0 to deny a capability, 1 to allow it.
/// - allow_fs: 1 = allow filesystem access, 0 = deny
/// - allow_net: 1 = allow TCP/HTTP networking, 0 = deny
/// - allow_exec: 1 = allow process execution, 0 = deny
/// - allow_ffi: 1 = allow FFI library loading, 0 = deny
/// - allow_sqlite: 1 = allow SQLite access, 0 = deny
#[no_mangle]
pub extern "C" fn pblang_create_vm_sandboxed(
    allow_fs: i32,
    allow_net: i32,
    allow_exec: i32,
    allow_ffi: i32,
    allow_sqlite: i32,
) -> *mut VmWrapper {
    let config = StdlibConfig {
        core: true,
        fs: allow_fs != 0,
        net: allow_net != 0,
        http: allow_net != 0,
        json: true,
        crypto: true,
        sqlite: allow_sqlite != 0,
        ffi: allow_ffi != 0,
        exec: allow_exec != 0,
        gfx: false,
        spawn: true,
        // Embedders that haven't opted into env reads shouldn't leak host secrets;
        // tie it to the exec capability (the closest "host access" gate they set).
        env: allow_exec != 0,
    };
    let w = VmWrapper {
        vm: VM::with_stdlib(config),
        last_error: CString::new("").unwrap(),
    };
    Box::into_raw(Box::new(w))
}

/// Set resource limits on a VM.
/// Pass 0 to leave a limit unset (unlimited).
/// - max_instructions: max bytecode instructions before aborting (0 = unlimited)
/// - max_stack_depth:   max call stack frames (0 = unlimited)
///
/// Returns 0 on success, 1 if vm is null.
///
/// # Safety
/// The `vm` pointer must be valid and created by a pblang_create_vm* function.
#[no_mangle]
pub unsafe extern "C" fn pblang_set_limits(
    vm: *mut VmWrapper,
    max_instructions: u64,
    max_stack_depth: usize,
) -> i32 {
    if vm.is_null() {
        return 1;
    }
    // SAFETY: Caller guarantees vm is a valid VmWrapper pointer.
    let w = &mut *vm;
    use pblang_vm::VmLimits;
    let limits = VmLimits {
        max_instructions: if max_instructions > 0 {
            Some(max_instructions)
        } else {
            None
        },
        max_stack_depth: if max_stack_depth > 0 {
            Some(max_stack_depth)
        } else {
            None
        },
        ..Default::default()
    };
    w.vm.set_limits(Some(limits));
    0
}

/// Returns the pblang runtime version string (UTF-8, NUL-terminated, static lifetime).
#[no_mangle]
pub extern "C" fn pblang_get_version() -> *const std::ffi::c_char {
    // Compile-time package version with explicit NUL terminator.
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const std::ffi::c_char
}

/// Call a named function defined in previously evaluated pblang source.
/// - fn_name: NUL-terminated function name
/// - args_json: NUL-terminated JSON array of arguments (e.g. `[42, "hello"]`)
/// - out: if non-null, receives a newly-allocated C string with the JSON-encoded return value.
///   Caller must free with pblang_string_free.
///
/// Returns 0 on success, non-zero on error.
///
/// # Safety
/// All pointers must be valid. fn_name and args_json must be valid NUL-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn pblang_call_function(
    vm: *mut VmWrapper,
    fn_name: *const std::ffi::c_char,
    args_json: *const std::ffi::c_char,
    out: *mut *mut std::ffi::c_char,
) -> i32 {
    if vm.is_null() || fn_name.is_null() {
        return 2;
    }
    // SAFETY: Checked non-null above; caller guarantees valid C strings.
    let w = &mut *vm;
    let name = std::ffi::CStr::from_ptr(fn_name)
        .to_string_lossy()
        .to_string();
    let args_str = if args_json.is_null() {
        "[]".to_string()
    } else {
        std::ffi::CStr::from_ptr(args_json)
            .to_string_lossy()
            .to_string()
    };

    // Parse args as JSON array — capped at the VM's `max_external_json_bytes`
    // (default 10 MB) to bound memory consumption from untrusted external
    // embedders. The cap matches what the IDE-server middleware enforces.
    let args_val: serde_json::Value = match pblang_vm::bounded_json_parse::<
        serde_json::Value,
    >("pblang_call_function args_json", args_str.as_bytes(), w.vm.limits())
    {
        Ok(v) => v,
        Err(e) => {
            set_error(w, e);
            return 1;
        }
    };

    let args_arr = match args_val {
        serde_json::Value::Array(a) => a,
        _ => {
            set_error(
                w,
                "pblang_call_function: args_json must be a JSON array, e.g. [1, \"x\"]",
            );
            return 1;
        }
    };

    // Reduce script-injection risk by constraining function selectors to simple identifiers
    // (with optional module-style dots).
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        || name.starts_with('.')
        || name.ends_with('.')
        || name.contains("..")
    {
        set_error(
            w,
            "pblang_call_function: fn_name must match [A-Za-z0-9_.] and not contain empty segments",
        );
        return 1;
    }

    let mut arg_exprs = Vec::with_capacity(args_arr.len());
    for arg in &args_arr {
        match json_to_pblang_expr(arg) {
            Ok(expr) => arg_exprs.push(expr),
            Err(e) => {
                set_error(
                    w,
                    format!("pblang_call_function: invalid args_json value: {e}"),
                );
                return 1;
            }
        }
    }
    let call_src = format!(
        "let __result = {name}({}); print __result;",
        arg_exprs.join(", ")
    );

    match parse(&call_src) {
        Ok(stmts) => {
            let chunk = match compile(stmts) {
                Ok(c) => c,
                Err(e) => {
                    set_error(w, format!("compile: {e}"));
                    return 1;
                }
            };
            match w.vm.run_collect(chunk) {
                Ok(values) => {
                    clear_error(w);
                    if !out.is_null() {
                        let s = values
                            .into_iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let c = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
                        // SAFETY: Checked out non-null above.
                        *out = c.into_raw();
                    }
                    0
                }
                Err(e) => {
                    set_error(w, format!("{:?}", e));
                    1
                }
            }
        }
        Err(errs) => {
            set_error(w, format!("parse error ({})", errs.len()));
            1
        }
    }
}

/// Destroy a pblang VM created by `pblang_create_vm`.
///
/// # Safety
/// The `vm` pointer must have been returned by `pblang_create_vm` and must not
/// have been freed already. After this call, the pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn pblang_free_vm(vm: *mut VmWrapper) {
    if vm.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `vm` was allocated by `pblang_create_vm` via
    // `Box::into_raw` and has not been freed. We take ownership back and drop it.
    drop(Box::from_raw(vm));
}

/// Returns a pointer to the last error message (UTF-8, NUL-terminated).
/// The returned pointer is owned by the VM and remains valid until the next FFI call on the same VM.
///
/// # Safety
/// The `vm` pointer must be valid and must have been created by `pblang_create_vm`.
#[no_mangle]
pub unsafe extern "C" fn pblang_last_error(vm: *mut VmWrapper) -> *const c_char {
    if vm.is_null() {
        return ptr::null();
    }
    // SAFETY: Caller guarantees `vm` is valid. The returned pointer is valid
    // until the next FFI call on this VM that might modify `last_error`.
    (*vm).last_error.as_ptr()
}

/// Evaluate a source string.
///
/// - On success returns 0 and, if `out` is non-null, writes a newly allocated C string containing
///   all printed values joined by newlines. Caller must free with `pblang_string_free`.
/// - On failure returns non-zero and sets `pblang_last_error`.
///
/// # Safety
/// - `vm` must be a valid pointer created by `pblang_create_vm`.
/// - `source` must be a valid NUL-terminated C string.
/// - If `out` is non-null, it must be a valid pointer to a `*mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn pblang_eval(
    vm: *mut VmWrapper,
    source: *const c_char,
    out: *mut *mut c_char,
) -> i32 {
    if vm.is_null() || source.is_null() {
        return 2;
    }

    // SAFETY: We checked `vm` is not null; caller guarantees it was created by pblang_create_vm.
    let w = &mut *vm;
    // SAFETY: We checked `source` is not null; caller guarantees it's a valid C string.
    let src = CStr::from_ptr(source).to_string_lossy().to_string();

    match parse(&src) {
        Ok(stmts) => {
            let chunk = match compile(stmts) {
                Ok(c) => c,
                Err(e) => {
                    set_error(w, format!("compile error: {}", e));
                    return 1;
                }
            };
            match w.vm.run_collect(chunk) {
                Ok(values) => {
                    clear_error(w);
                    if !out.is_null() {
                        let s = values
                            .into_iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let c = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
                        // SAFETY: We checked `out` is not null above; caller guarantees
                        // it points to valid memory for a `*mut c_char`.
                        *out = c.into_raw();
                    }
                    0
                }
                Err(e) => {
                    set_error(w, format!("{:?}", e));
                    1
                }
            }
        }
        Err(errs) => {
            set_error(w, format!("parse error ({})", errs.len()));
            1
        }
    }
}

/// Free a string allocated by pblang FFI.
///
/// # Safety
/// The `s` pointer must have been returned by a pblang FFI function (e.g., via `pblang_eval`)
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn pblang_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `s` was allocated by `CString::into_raw` in this crate
    // and has not been freed. We take ownership back and drop it.
    drop(CString::from_raw(s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_version_matches_package_version() {
        let ptr = pblang_get_version();
        assert!(!ptr.is_null(), "pblang_get_version returned null");
        let actual = unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .expect("version must be valid UTF-8");
        assert_eq!(actual, env!("CARGO_PKG_VERSION"));
    }
}
