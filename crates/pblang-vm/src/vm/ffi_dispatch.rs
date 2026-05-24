//! FFI signature parsing and `extern "C"` trampoline dispatch.
//!
//! Split out of `vm.rs` so the FFI surface lives in one auditable file. The
//! VM side of things (the `ffi_load` / `ffi_call` / `ffi_call_typed`
//! builtins, the allowlist enforcement) stays in `vm.rs`; everything below
//! is pure dispatch — given a parsed signature key like `"i,s->i"` and
//! pre-marshalled argument arrays, call the symbol with the right `unsafe
//! extern "C"` trampoline and box the return value.
//!
//! Adding a new signature shape is local to this file.
//
// All functions are `pub(super)` so they're reachable from the parent `vm`
// module but invisible outside the crate.

use libloading::Library;
use pblang_ir::{alloc_string, Value};

/// Parse an FFI signature string of the form `"arg1,arg2,...->ret"` into
/// `(arg_types, ret_type)`. Both sides are validated against the supported
/// type set; an unknown type yields a friendly error.
///
/// Supported arg types: `i64 | i32 | f64 | f32 | bool | string | ptr`.
/// Supported ret types: same set plus `void`.
pub(super) fn parse_ffi_sig(s: &str) -> Result<(Vec<String>, String), String> {
    let parts: Vec<&str> = s.split("->").collect();
    if parts.len() != 2 {
        return Err(format!("signature must be of the form 'a,b->c', got '{s}'"));
    }
    let arg_part = parts[0].trim();
    let ret_part = parts[1].trim();
    let args: Vec<String> = if arg_part.is_empty() {
        vec![]
    } else {
        arg_part.split(',').map(|s| s.trim().to_string()).collect()
    };
    if ret_part.is_empty() {
        return Err("missing return type after '->'".into());
    }
    let ret = ret_part.to_string();
    for a in &args {
        if !matches!(
            a.as_str(),
            "i64" | "i32" | "f64" | "f32" | "bool" | "string" | "ptr"
        ) {
            return Err(format!("unknown arg type '{a}'"));
        }
    }
    if !matches!(
        ret.as_str(),
        "i64" | "i32" | "f64" | "f32" | "bool" | "string" | "ptr" | "void"
    ) {
        return Err(format!("unknown ret type '{ret}'"));
    }
    Ok((args, ret))
}

/// Dispatch an FFI call given the parsed signature `key` like `"i,s->i"` and
/// already-typed argument arrays. Returns the result as a pblang Value.
///
/// SAFETY: caller must guarantee that the dynamically loaded `sym` in `lib`
/// matches the signature represented by `key`. Mismatch is undefined behavior
/// in the underlying C library — this is no different from raw `libloading`.
pub(super) unsafe fn dispatch_ffi(
    lib: &Library,
    sym: &str,
    key: &str,
    a_i: &[i64; 4],
    a_d: &[f64; 4],
    a_s: &[*const libc::c_char; 4],
) -> Result<Value, String> {
    // We encode types as i=integer/bool/ptr, d=double, s=string, v=void.
    // Supported shapes: explicit list keeps trampoline count bounded.
    let bytes = sym.as_bytes();
    let get_err = |e: libloading::Error| format!("{e}");
    match key {
        // 0-arg returns
        "->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn() -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f() as f64))
        }
        "->d" => {
            let f: libloading::Symbol<unsafe extern "C" fn() -> f64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f()))
        }
        "->s" => {
            let f: libloading::Symbol<unsafe extern "C" fn() -> *const libc::c_char> =
                lib.get(bytes).map_err(get_err)?;
            let p = f();
            Ok(c_str_to_value(p))
        }
        "->v" => {
            let f: libloading::Symbol<unsafe extern "C" fn()> = lib.get(bytes).map_err(get_err)?;
            f();
            Ok(Value::Nil)
        }
        // 1-arg shapes
        "i->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_i[0]) as f64))
        }
        "i->d" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64) -> f64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_i[0])))
        }
        "i->s" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64) -> *const libc::c_char> =
                lib.get(bytes).map_err(get_err)?;
            Ok(c_str_to_value(f(a_i[0])))
        }
        "i->v" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64)> =
                lib.get(bytes).map_err(get_err)?;
            f(a_i[0]);
            Ok(Value::Nil)
        }
        "d->d" => {
            let f: libloading::Symbol<unsafe extern "C" fn(f64) -> f64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_d[0])))
        }
        "d->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(f64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_d[0]) as f64))
        }
        "s->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(*const libc::c_char) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_s[0]) as f64))
        }
        "s->s" => {
            let f: libloading::Symbol<
                unsafe extern "C" fn(*const libc::c_char) -> *const libc::c_char,
            > = lib.get(bytes).map_err(get_err)?;
            Ok(c_str_to_value(f(a_s[0])))
        }
        "s->v" => {
            let f: libloading::Symbol<unsafe extern "C" fn(*const libc::c_char)> =
                lib.get(bytes).map_err(get_err)?;
            f(a_s[0]);
            Ok(Value::Nil)
        }
        // 2-arg shapes
        "i,i->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64, i64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_i[0], a_i[1]) as f64))
        }
        "d,d->d" => {
            let f: libloading::Symbol<unsafe extern "C" fn(f64, f64) -> f64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_d[0], a_d[1])))
        }
        "s,s->i" => {
            let f: libloading::Symbol<
                unsafe extern "C" fn(*const libc::c_char, *const libc::c_char) -> i64,
            > = lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_s[0], a_s[1]) as f64))
        }
        "s,s->s" => {
            let f: libloading::Symbol<
                unsafe extern "C" fn(
                    *const libc::c_char,
                    *const libc::c_char,
                ) -> *const libc::c_char,
            > = lib.get(bytes).map_err(get_err)?;
            Ok(c_str_to_value(f(a_s[0], a_s[1])))
        }
        "i,s->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64, *const libc::c_char) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_i[0], a_s[1]) as f64))
        }
        "s,i->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(*const libc::c_char, i64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_s[0], a_i[1]) as f64))
        }
        // 3-arg uniform shapes
        "i,i,i->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64, i64, i64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_i[0], a_i[1], a_i[2]) as f64))
        }
        "d,d,d->d" => {
            let f: libloading::Symbol<unsafe extern "C" fn(f64, f64, f64) -> f64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_d[0], a_d[1], a_d[2])))
        }
        "s,s,s->i" => {
            let f: libloading::Symbol<
                unsafe extern "C" fn(
                    *const libc::c_char,
                    *const libc::c_char,
                    *const libc::c_char,
                ) -> i64,
            > = lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(f(a_s[0], a_s[1], a_s[2]) as f64))
        }
        // 4-arg shapes
        "i,i,i,i->i" => {
            let f: libloading::Symbol<unsafe extern "C" fn(i64, i64, i64, i64) -> i64> =
                lib.get(bytes).map_err(get_err)?;
            Ok(Value::Number(
                f(a_i[0], a_i[1], a_i[2], a_i[3]) as f64,
            ))
        }
        other => Err(format!(
            "ffi_call_typed: unsupported signature dispatch '{other}'. Supported shapes are listed in the docstring. \
For unusual signatures, write a tiny C wrapper or use ffi_call (numeric)."
        )),
    }
}

/// Convert a possibly-null C string pointer into a pblang Value (Nil if null).
pub(super) fn c_str_to_value(p: *const libc::c_char) -> Value {
    if p.is_null() {
        Value::Nil
    } else {
        // SAFETY: caller (the FFI function) is responsible for returning a valid
        // null-terminated UTF-8/ASCII C string. We copy it into a Rust String
        // immediately so subsequent FFI calls cannot invalidate it.
        let s = unsafe { std::ffi::CStr::from_ptr(p) };
        match s.to_str() {
            Ok(st) => alloc_string(st.to_string()),
            Err(_) => alloc_string(String::from_utf8_lossy(s.to_bytes()).into_owned()),
        }
    }
}
