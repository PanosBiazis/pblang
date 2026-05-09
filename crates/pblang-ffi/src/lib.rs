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

/// Create a new Sapphire VM.
#[no_mangle]
pub extern "C" fn pblang_create_vm() -> *mut VmWrapper {
    let w = VmWrapper {
        vm: VM::with_stdlib(StdlibConfig::default()),
        last_error: CString::new("").unwrap(),
    };
    Box::into_raw(Box::new(w))
}

/// Destroy a Sapphire VM created by `pblang_create_vm`.
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
pub unsafe extern "C" fn pblang_eval(vm: *mut VmWrapper, source: *const c_char, out: *mut *mut c_char) -> i32 {
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

/// Free a string allocated by Sapphire FFI.
///
/// # Safety
/// The `s` pointer must have been returned by a Sapphire FFI function (e.g., via `pblang_eval`)
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
