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
//!   - `ffi_close(handle)` — revoke the handle; unload after active calls return.
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
//! UB per `docs/FFI_contract.md`. The Library is owned by the per-execution-realm
//! FFI table; `ffi_close` removes realm authority, while active calls retain it.

use libloading::Library;

use pblang_ir::{RuntimeError, Value, VmHost};

use super::errors::vm_cap;
use super::ffi_dispatch::{dispatch_ffi, parse_ffi_sig};
use super::handles::{
    insert_ffi_library, require_ffi_capacity, value_to_ffi_handle, with_ffi_table,
};
use super::FfiResource;

pub(super) fn ffi_load(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(vm_cap!(host, "ffi_load", "ffi"));
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
    require_ffi_capacity(host)?;
    let lib = unsafe { Library::new(path) }
        .map_err(|e| host.err(format!("ffi_load(): failed to load library {path:?}: {e}")))?;
    let id = insert_ffi_library(host, lib)?;
    Ok(Value::Number(id as f64))
}

pub(super) fn ffi_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(vm_cap!(host, "ffi_close", "ffi"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "ffi_close() expects 1 argument (handle), got {}",
            args.len()
        )));
    }
    let id = value_to_ffi_handle(host, args[0].clone(), "ffi_close")?;
    let resource = with_ffi_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("ffi_close(): invalid handle".into()))?;
    drop(resource);
    Ok(Value::Nil)
}

pub(super) fn ffi_call(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_ffi() {
        return Err(vm_cap!(host, "ffi_call", "ffi"));
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
    let id = value_to_ffi_handle(host, args[0].clone(), "ffi_call")?;
    let arr = match &args[2] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("ffi_call(): expected args array".into())),
    };

    if arr.borrow().len() > 6 {
        return Err(host.err("ffi_call(): too many args, max 6".into()));
    }
    let mut nums: Vec<f64> = vec![];
    for v in arr.borrow().iter() {
        match v {
            Value::Number(n) => nums.push(*n),
            _ => return Err(host.err("ffi_call(): args must be numbers for now".into())),
        }
    }

    let res = match with_ffi_table(host, |t| t.get(&id).cloned())? {
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
                        let f: libloading::Symbol<unsafe extern "C" fn() -> f64> =
                            lib.get(sym.as_bytes()).map_err(|e| {
                                host.err(format!(
                                    "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                                ))
                            })?;
                        f()
                    }
                    1 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64) -> f64> =
                            lib.get(sym.as_bytes()).map_err(|e| {
                                host.err(format!(
                                    "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                                ))
                            })?;
                        f(nums[0])
                    }
                    2 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64, f64) -> f64> =
                            lib.get(sym.as_bytes()).map_err(|e| {
                                host.err(format!(
                                    "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                                ))
                            })?;
                        f(nums[0], nums[1])
                    }
                    3 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64, f64, f64) -> f64> =
                            lib.get(sym.as_bytes()).map_err(|e| {
                                host.err(format!(
                                    "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                                ))
                            })?;
                        f(nums[0], nums[1], nums[2])
                    }
                    4 => {
                        let f: libloading::Symbol<unsafe extern "C" fn(f64, f64, f64, f64) -> f64> =
                            lib.get(sym.as_bytes()).map_err(|e| {
                                host.err(format!(
                                    "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                                ))
                            })?;
                        f(nums[0], nums[1], nums[2], nums[3])
                    }
                    5 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64, f64, f64) -> f64,
                        > = lib.get(sym.as_bytes()).map_err(|e| {
                            host.err(format!(
                                "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                            ))
                        })?;
                        f(nums[0], nums[1], nums[2], nums[3], nums[4])
                    }
                    6 => {
                        let f: libloading::Symbol<
                            unsafe extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64,
                        > = lib.get(sym.as_bytes()).map_err(|e| {
                            host.err(format!(
                                "ffi_call(): symbol {sym:?} not found in loaded library: {e}"
                            ))
                        })?;
                        f(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5])
                    }
                    n => {
                        return Err(host.err(format!("ffi_call(): too many args ({}), max 6", n)));
                    }
                }
            };
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
        return Err(vm_cap!(host, "ffi_call_typed", "ffi"));
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
    let id = value_to_ffi_handle(host, args[0].clone(), "ffi_call_typed")?;
    let arr = match &args[3] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("ffi_call_typed(): expected args array".into())),
    };

    // Parse signature.
    let (arg_tys, ret_ty) =
        parse_ffi_sig(&sig_str).map_err(|m| host.err(format!("ffi_call_typed(): {m}")))?;
    if arr.borrow().len() > 4 {
        return Err(host.err("ffi_call_typed(): up to 4 arguments supported per call".into()));
    }
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
                        return Err(host.err(format!("ffi_call_typed(): arg[{i}] expected number")))
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
            _ => return Err(host.err(format!("ffi_call_typed(): unsupported arg type '{ty}'"))),
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

    let res = match with_ffi_table(host, |t| t.get(&id).cloned())? {
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

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod tests {
    use super::super::handles::MAX_FFI_LIBRARIES_PER_REALM;
    use super::super::{StdlibConfig, VM};
    use super::*;
    use pblang_ir::alloc_string;
    use std::rc::Rc;

    const MISSING: &str = "pblang_test_nonexistent_ffi_symbol_ownership";

    fn vm() -> VM {
        VM::with_stdlib(StdlibConfig {
            ffi: true,
            ffi_allowed_libs: ["libc.so.6".into()].into_iter().collect(),
            ffi_allowed_symbols: [MISSING.into()].into_iter().collect(),
            ..Default::default()
        })
    }

    fn load(vm: &mut VM) -> Value {
        ffi_load(vm, vec![alloc_string("libc.so.6")]).unwrap()
    }

    fn call(vm: &mut VM, handle: Value, typed: bool) -> Result<Value, RuntimeError> {
        let mut args = vec![handle, alloc_string(MISSING)];
        if typed {
            args.push(alloc_string("->i64"));
        }
        args.push(super::super::alloc_array(vec![]));
        if typed {
            ffi_call_typed_builtin(vm, args)
        } else {
            ffi_call(vm, args)
        }
    }

    fn run(vm: &mut VM, source: &str) -> Result<Vec<Value>, RuntimeError> {
        let chunk = pblang_compiler::compile(pblang_core::parse(source).unwrap()).unwrap();
        vm.run_collect(chunk)
    }

    #[test]
    fn ffi_root_realms_reject_foreign_call_and_close() {
        let (mut a, mut b) = (vm(), vm());
        let ha = load(&mut a);
        let hb = load(&mut b);
        for typed in [false, true] {
            assert!(call(&mut b, ha.clone(), typed)
                .unwrap_err()
                .message
                .contains("invalid handle"));
            assert!(call(&mut a, hb.clone(), typed)
                .unwrap_err()
                .message
                .contains("invalid handle"));
        }
        assert!(ffi_close(&mut b, vec![ha.clone()])
            .unwrap_err()
            .message
            .contains("invalid handle"));
        ffi_close(&mut a, vec![ha.clone()]).unwrap();
        assert!(ffi_close(&mut a, vec![ha])
            .unwrap_err()
            .message
            .contains("invalid handle"));
        drop(a);
        ffi_close(&mut b, vec![hb]).unwrap();
    }

    #[test]
    fn ffi_missing_symbol_preserves_handle_and_final_owner_drop_releases_library() {
        let mut owner = vm();
        let handle = load(&mut owner);
        for typed in [false, true] {
            let err = call(&mut owner, handle.clone(), typed).unwrap_err();
            assert!(!err.message.contains("invalid handle"), "{}", err.message);
            assert!(err.message.contains(MISSING), "{}", err.message);
        }
        let id = value_to_ffi_handle(&mut owner, handle.clone(), "test").unwrap();
        let weak = with_ffi_table(&mut owner, |table| {
            let FfiResource::Library(lib) = table.get(&id).unwrap();
            Rc::downgrade(lib)
        })
        .unwrap();
        assert!(weak.upgrade().is_some());
        ffi_close(&mut owner, vec![handle]).unwrap();
        assert!(weak.upgrade().is_none());
        let handle = load(&mut owner);
        let id = value_to_ffi_handle(&mut owner, handle, "test").unwrap();
        let weak = with_ffi_table(&mut owner, |table| {
            let FfiResource::Library(lib) = table.get(&id).unwrap();
            Rc::downgrade(lib)
        })
        .unwrap();
        drop(owner);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn ffi_handles_reject_lossy_and_malformed_numbers() {
        let mut owner = vm();
        for n in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            1.5,
            9007199254740992.0,
        ] {
            assert!(ffi_close(&mut owner, vec![Value::Number(n)])
                .unwrap_err()
                .message
                .contains("expected exact"));
        }
        assert!(ffi_close(&mut owner, vec![alloc_string("1")]).is_err());
        assert_eq!(
            value_to_ffi_handle(&mut owner, Value::Number(9007199254740991.0), "test").unwrap(),
            9007199254740991
        );
    }

    #[test]
    fn ffi_capacity_rejects_before_loader_or_polyglot_effects() {
        let mut owner = vm();
        let handles: Vec<_> = (0..MAX_FFI_LIBRARIES_PER_REALM)
            .map(|_| load(&mut owner))
            .collect();
        // Allowing a deliberately absent path makes a loader attempt distinguishable
        // from the required capacity rejection, without running a constructor.
        owner.ffi_policy.as_mut().unwrap().allow_all = true;
        let err = ffi_load(
            &mut owner,
            vec![alloc_string("./pblang-missing-capacity-probe.so")],
        )
        .unwrap_err();
        assert!(err.message.contains("library limit exceeded"));
        owner.stdlib_config.as_mut().unwrap().exec = true;
        let err = super::super::polyglot::polyglot_compile_cached(
            &mut owner,
            vec![alloc_string("c"), alloc_string("not compilable")],
        )
        .unwrap_err();
        assert!(err.message.contains("library limit exceeded"));
        ffi_close(&mut owner, vec![handles[0].clone()]).unwrap();
        load(&mut owner);
        assert_eq!(
            owner.ffi_resources.borrow().len(),
            MAX_FFI_LIBRARIES_PER_REALM
        );
    }

    #[test]
    fn ffi_nested_import_keeps_root_realm_alive() {
        let dir =
            std::env::temp_dir().join(format!("pblang-ffi-import-{:016x}", rand::random::<u64>()));
        std::fs::create_dir(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let inner = dir.join("inner.sapp");
        let outer = dir.join("outer.sapp");
        std::fs::write(&inner, "let h = ffi_load(\"libc.so.6\"); export h;").unwrap();
        std::fs::write(
            &outer,
            format!(
                "let m = import({}); let h = m.h; export h;",
                serde_json::to_string(&inner).unwrap()
            ),
        )
        .unwrap();
        let mut owner = vm();
        let source = format!(
            "let m = import({}); ffi_close(m.h); print 42;",
            serde_json::to_string(&outer).unwrap()
        );
        assert_eq!(run(&mut owner, &source).unwrap(), vec![Value::Number(42.0)]);
        assert!(owner.ffi_resources.borrow().is_empty());
        assert!(run(&mut owner, "ffi_close(m.h);")
            .unwrap_err()
            .message
            .contains("invalid handle"));
    }

    #[test]
    fn ffi_spawn_cannot_use_parent_handle() {
        let mut owner = vm();
        let source = r#"
            let h = ffi_load("libc.so.6");
            let worker = spawn(fn() { ffi_close(h); });
            join(worker);
        "#;
        assert!(run(&mut owner, source)
            .unwrap_err()
            .message
            .contains("invalid handle"));
        assert_eq!(owner.ffi_resources.borrow().len(), 1);
        run(&mut owner, "ffi_close(h);").unwrap();
    }
}
