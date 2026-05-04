// G:\panag\Documents\projects\AAP\pl-dev\sapphire\crates\sapphire-ffi\src\lib.rs
use std::ffi::{c_char, CStr, CString};
use std::ptr;

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

pub(crate) struct VmWrapper {
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
#[no_mangle]
pub extern "C" fn pblang_free_vm(vm: *mut VmWrapper) {
    if vm.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(vm));
    }
}

/// Returns a pointer to the last error message (UTF-8, NUL-terminated).
/// The returned pointer is owned by the VM and remains valid until the next FFI call on the same VM.
#[no_mangle]
pub extern "C" fn pblang_last_error(vm: *mut VmWrapper) -> *const c_char {
    if vm.is_null() {
        return ptr::null();
    }
    unsafe { (*vm).last_error.as_ptr() }
}

/// Evaluate a source string.
///
/// - On success returns 0 and, if `out` is non-null, writes a newly allocated C string containing
///   all printed values joined by newlines. Caller must free with `pblang_string_free`.
/// - On failure returns non-zero and sets `pblang_last_error`.
#[no_mangle]
pub extern "C" fn pblang_eval(vm: *mut VmWrapper, source: *const c_char, out: *mut *mut c_char) -> i32 {
    if vm.is_null() || source.is_null() {
        return 2;
    }

    let w = unsafe { &mut *vm };
    let src = unsafe { CStr::from_ptr(source) }.to_string_lossy().to_string();

    match parse(&src) {
        Ok(stmts) => {
            let chunk = compile(stmts);
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
                        unsafe {
                            *out = c.into_raw();
                        }
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
#[no_mangle]
pub extern "C" fn pblang_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

