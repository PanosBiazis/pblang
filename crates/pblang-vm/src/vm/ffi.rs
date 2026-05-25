//! FFI builtin implementations.
//!
//! Free functions backing the `ffi_*` builtins registered in
//! `VM::builtins()` (vm.rs). They are the user-facing pblang surface for
//! `dlopen`/`dlsym`/call — the heavy lifting (signature parsing,
//! per-arity transmutes, c-string lifetime management) lives in the
//! sibling `ffi_dispatch` module.
//!
//!   - `ffi_load(path)` → handle. Gated by `host.allow_ffi()` AND by
//!     `host.ffi_allowed_library(path)` (default deny).
//!   - `ffi_close(handle)` — drop the Library, run its destructors.
//!   - `ffi_call(handle, "sym", [nums…])` → number. The simple
//!     numeric-only path; supports up to 6 `f64` args returning `f64`.
//!   - `ffi_call_typed(handle, "sym", "sig", [args…])` → typed value.
//!     `sig` is `"<arg-types>-><ret-type>"` with comma-separated parts;
//!     see the `ffi_dispatch` module docs for the supported types and
//!     dispatch shapes.
//!
//! ## SAFETY
//!
//! Loading a user-named shared library executes any constructors /
//! `_init`-style code in this process. Every call site checks
//! `host.ffi_allowed_library()` (default deny) before `Library::new`,
//! and the whole surface is gated by `host.allow_ffi()`. Resolved
//! symbols are called as `extern "C" fn` with the user-claimed
//! signature — if the library's actual export disagrees, behaviour is
//! UB per `docs/FFI_contract.md`. The Library is owned by the per-VM
//! FFI table; `ffi_close` removes it (and drops it).

use std::sync::atomic::Ordering;

use libloading::Library;

use pblang_ir::{RuntimeError, Value, VmHost};

use super::ffi_dispatch::{dispatch_ffi, parse_ffi_sig};
use super::handles::{value_to_handle, with_ffi_table, FFI_NEXT};
use super::FfiResource;

pub(super) fn ffi_load(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(host.err("ffi access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "ffi_load() expects 1 argument (path), got {}",
            args.len()
        )));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| host.err(format!("ffi_load(): {m}")))?;
    if !host.ffi_allowed_library(path) {
        return Err(host.err(format!(
            "ffi_load: library {path:?} not in allowlist (set FfiPolicy.allowed_libs or allow_all=true)"
        )));
    }
    // SAFETY: Loading a user-named shared library executes any
    // constructors / `_init`-style code in this process. `path` was
    // checked against `host.ffi_allowed_library()` immediately above
    // (defaults deny everything), and the whole builtin is gated by
    // `host.allow_ffi()`. The returned Library is owned by the FFI
    // table; drop = unload.
    let lib =
        unsafe { Library::new(path) }.map_err(|e| host.err(format!("ffi_load(): {e}")))?;
    let id = with_ffi_table(host, |t| {
        let id = FFI_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(id, FfiResource::Library(lib));
        id
    })?;
    Ok(Value::Number(id as f64))
}

pub(super) fn ffi_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(host.err("ffi access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "ffi_close() expects 1 argument (handle), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "ffi_close")?;
    let _ = with_ffi_table(host, |t| t.remove(&id));
    Ok(Value::Nil)
}

pub(super) fn ffi_call(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(host.err("ffi access denied".into()));
    }
    // ffi_call(handle, "symbol", [args...]) -> number (f64)
    if args.len() != 3 {
        return Err(host.err(format!(
            "ffi_call() expects 3 arguments (handle, symbol, args_array), got {}",
            args.len()
        )));
    }
    let sym = args[1]
        .as_str()
        .map_err(|m| host.err(format!("ffi_call(): {m}")))?;
    if !host.ffi_allowed_symbol(sym) {
        return Err(host.err(format!(
            "ffi_call: symbol {sym:?} not in allowlist (set FfiPolicy.allowed_symbols or allow_all=true)"
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "ffi_call")?;
    let arr = match &args[2] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("ffi_call(): expected args array".into())),
    };

    let mut nums: Vec<f64> = vec![];
    for v in arr.borrow().iter() {
        match v {
            Value::Number(n) => nums.push(*n),
            _ => return Err(host.err("ffi_call(): args must be numbers for now".into())),
        }
    }

    let res = match with_ffi_table(host, |t| t.remove(&id))? {
        Some(FfiResource::Library(lib)) => {
            // SAFETY: We resolve `sym` from the library via
            // libloading's `get`, then call the function pointer.
            // Each match arm below specifies the exact `extern "C" fn`
            // signature we're treating the symbol as — if the
            // library's actual export disagrees, behaviour is UB per
            // the FFI contract documented at the top of this module
            // (`docs/FFI_contract.md`). The builtin is gated by
            // `host.allow_ffi()` and the user-supplied `id` is the
            // table slot for a Library *they* loaded via `ffi_load`
            // (which checked the allowlist).
            let out = unsafe {
                match nums.len() {
                    0 => {
                        let f: libloading::Symbol<unsafe extern "C" fn() -> f64> = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f()
                    }
                    1 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64) -> f64> = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0])
                    }
                    2 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64, f64) -> f64> =
                            lib.get(sym.as_bytes())
                                .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0], nums[1])
                    }
                    3 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64) -> f64,
                        > = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0], nums[1], nums[2])
                    }
                    4 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64, f64) -> f64,
                        > = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0], nums[1], nums[2], nums[3])
                    }
                    5 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64, f64, f64) -> f64,
                        > = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0], nums[1], nums[2], nums[3], nums[4])
                    }
                    6 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64,
                        > = lib
                            .get(sym.as_bytes())
                            .map_err(|e| host.err(format!("ffi_call(): {e}")))?;
                        f(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5])
                    }
                    n => {
                        let _ = with_ffi_table(host, |t| {
                            t.insert(id, FfiResource::Library(lib))
                        });
                        return Err(
                            host.err(format!("ffi_call(): too many args ({}), max 6", n))
                        );
                    }
                }
            };
            let _ = with_ffi_table(host, |t| t.insert(id, FfiResource::Library(lib)));
            Ok(Value::Number(out))
        }
        None => Err(host.err("ffi_call(): invalid handle".into())),
    }?;

    Ok(res)
}

// ── FFI typed-signature call ──────────────────────────────────────────
//
// ffi_call_typed(handle, "symbol", "sig", [args...]) -> value
//
// The `sig` argument is a string of the form `"<arg-types>-><ret-type>"`,
// where each side is comma-separated. Supported types:
//   i64       — signed 64-bit integer (passed as `int64_t`)
//   i32       — signed 32-bit integer (passed as `int32_t`)
//   f64       — double (passed as `double`)
//   f32       — float (passed as `float`)
//   bool      — `int32_t` (1/0)
//   string    — `const char*` (UTF-8, null-terminated)
//   ptr       — opaque `*const c_void`, passed as the integer handle (i64)
//   void      — only valid for the return position (no value)
//
// For now we support the most common dispatch shapes used by C
// libraries: up to 4 arguments, with uniform-or-mixed types. We pick a
// statically-typed transmute per signature; longer / exotic signatures
// should still go through `ffi_call` (numeric) or an explicit binding
// crate.
pub(super) fn ffi_call_typed_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(host.err("ffi access denied".into()));
    }
    if args.len() != 4 {
        return Err(host.err(format!(
            "ffi_call_typed() expects 4 arguments (handle, symbol, sig, args_array), got {}",
            args.len()
        )));
    }
    let sym = args[1]
        .as_str()
        .map_err(|m| host.err(format!("ffi_call_typed(): {m}")))?
        .to_string();
    if !host.ffi_allowed_symbol(&sym) {
        return Err(host.err(format!(
            "ffi_call_typed: symbol {sym:?} not in allowlist (set FfiPolicy.allowed_symbols or allow_all=true)"
        )));
    }
    let sig_str = args[2]
        .as_str()
        .map_err(|m| host.err(format!("ffi_call_typed(): {m}")))?
        .to_string();
    let id = value_to_handle(host, args[0].clone(), "ffi_call_typed")?;
    let arr = match &args[3] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("ffi_call_typed(): expected args array".into())),
    };

    // Parse signature.
    let (arg_tys, ret_ty) =
        parse_ffi_sig(&sig_str).map_err(|m| host.err(format!("ffi_call_typed(): {m}")))?;
    let raw_args = arr.borrow().clone();
    if raw_args.len() != arg_tys.len() {
        return Err(host.err(format!(
            "ffi_call_typed(): signature expects {} args, got {}",
            arg_tys.len(),
            raw_args.len()
        )));
    }

    // Convert pblang values into typed C arguments. Lifetimes for CStrings
    // are held in `c_strings` for the duration of the call.
    let mut c_strings: Vec<std::ffi::CString> = vec![];
    let mut a_i64 = [0i64; 4];
    let mut a_f64 = [0.0f64; 4];
    let mut a_ptr = [std::ptr::null::<libc::c_char>(); 4];
    let n = arg_tys.len();
    if n > 4 {
        return Err(host.err(
            "ffi_call_typed(): up to 4 arguments supported per call (use multiple smaller wrappers for more)".into(),
        ));
    }
    for (i, ty) in arg_tys.iter().enumerate() {
        let v = &raw_args[i];
        match ty.as_str() {
            "i64" | "i32" | "bool" | "ptr" => {
                a_i64[i] = match v {
                    Value::Number(n) => *n as i64,
                    Value::Bool(b) => *b as i64,
                    _ => {
                        return Err(host.err(format!(
                            "ffi_call_typed(): arg[{i}] expected integer/bool/ptr"
                        )))
                    }
                };
            }
            "f64" | "f32" => {
                a_f64[i] = match v {
                    Value::Number(n) => *n,
                    _ => {
                        return Err(
                            host.err(format!("ffi_call_typed(): arg[{i}] expected number"))
                        )
                    }
                };
            }
            "string" => {
                let s = v
                    .as_str()
                    .map_err(|m| host.err(format!("ffi_call_typed(): arg[{i}] {m}")))?;
                let c = std::ffi::CString::new(s).map_err(|_| {
                    host.err(format!("ffi_call_typed(): arg[{i}] contained NUL byte"))
                })?;
                a_ptr[i] = c.as_ptr();
                c_strings.push(c);
            }
            _ => {
                return Err(
                    host.err(format!("ffi_call_typed(): unsupported arg type '{ty}'"))
                )
            }
        }
    }

    // Build a uniform signature string for dispatch: "i,s,i->i".
    let abbrev = |t: &str| -> &'static str {
        match t {
            "i64" | "i32" | "bool" | "ptr" => "i",
            "f64" | "f32" => "d",
            "string" => "s",
            "void" => "v",
            _ => "?",
        }
    };
    let key: String = format!(
        "{}->{}",
        arg_tys
            .iter()
            .map(|t| abbrev(t))
            .collect::<Vec<_>>()
            .join(","),
        abbrev(&ret_ty)
    );

    let res = match with_ffi_table(host, |t| t.remove(&id))? {
        Some(FfiResource::Library(lib)) => {
            // SAFETY: `dispatch_ffi` resolves `sym` in `lib`, parses
            // `key` into a concrete C ABI signature, and invokes the
            // function pointer. `key` is derived from the user's
            // `signature` string after the parser validated arity
            // (≤6 args) and per-arg type tokens; the strings/ptrs in
            // `a_ptr` are CString-owned and outlive the call window.
            // Per the FFI contract, mismatch between `key` and the
            // library's actual signature is UB — the host is expected
            // to validate symbol exports before granting `allow_ffi`.
            let out_res = unsafe { dispatch_ffi(&lib, &sym, &key, &a_i64, &a_f64, &a_ptr) };
            let _ = with_ffi_table(host, |t| t.insert(id, FfiResource::Library(lib)));
            match out_res {
                Ok(v) => Ok(v),
                Err(e) => Err(host.err(format!("ffi_call_typed(): {e}"))),
            }
        }
        None => Err(host.err("ffi_call_typed(): invalid handle".into())),
    }?;
    // c_strings are dropped here, after the call returns.
    drop(c_strings);
    Ok(res)
}
