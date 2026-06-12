//! Runtime / I/O / introspection builtin implementations.
//!
//! Free functions backing the catch-all "runtime" builtins registered
//! in `VM::builtins()` (vm.rs) — the ones too small or too cross-cutting
//! to deserve their own submodule:
//!
//!   - **Process**: `proc_spawn_wait(cmd, args[])` → map<exit_code, stdout, stderr>
//!     (gated on `host.allow_exec()`); `exec(cmd)` — deprecated single-token
//!     no-shell variant kept for back-compat, rejects whitespace and shell
//!     metacharacters.
//!   - **I/O**: `read_line()` — reads from stdin (or VM's `pop_stdin_line`
//!     seed for the REPL); `import(path)` — delegates to `VmHost::import_module`.
//!   - **Conversion**: `str_(v)` → string via `Value::to_string`.
//!   - **Collections meta**: `len(v)` for strings/arrays/maps.
//!   - **GC**: `gc_collect()` — cycle-breaker pass via `VmHost::gc_collect_cycles`
//!     (does NOT clear native handle tables — those are ref-counted by user
//!     handles); `gc_stats()` — counter dump.
//!   - **Assertions**: `assert(cond[, msg])` — Rust-style debug check; raises
//!     `RuntimeError("AssertionError: …")` on falsy `cond`.
//!
//! Capability gates: only `proc_spawn_wait` and `exec` need `allow_exec`.

use std::sync::atomic::Ordering;

use pblang_ir::{alloc_string, string_alloc_count, Obj, RuntimeError, Value, VmHost};

use super::errors::vm_cap;
use super::{alloc_map, ARRAY_ALLOCS, MAP_ALLOCS, MODULE_ALLOCS, VM};

pub(super) fn proc_spawn_wait(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "proc_spawn_wait", "exec"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "proc_spawn_wait() expects 2 arguments (cmd, args), got {}",
            args.len()
        )));
    }
    let cmd = args[0]
        .as_str()
        .map_err(|m| host.err(format!("proc_spawn_wait(): {m}")))?;
    let argv = match &args[1] {
        Value::Array(arr) => {
            let mut v: Vec<String> = Vec::new();
            for item in arr.borrow().iter() {
                let s = item
                    .as_str()
                    .map_err(|m| host.err(format!("proc_spawn_wait(): arg: {m}")))?;
                v.push(s.to_string());
            }
            v
        }
        _ => return Err(host.err("proc_spawn_wait(): second argument must be an array".into())),
    };
    let output = std::process::Command::new(cmd)
        .args(&argv)
        .output()
        .map_err(|e| host.err(format!("proc_spawn_wait(): {e}")))?;
    let exit_code = output.status.code().unwrap_or(1) as f64;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut m = std::collections::HashMap::new();
    m.insert("exit_code".to_string(), Value::Number(exit_code));
    m.insert("stdout".to_string(), alloc_string(stdout));
    m.insert("stderr".to_string(), alloc_string(stderr));
    Ok(alloc_map(m))
}

pub(super) fn str_(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("str() expects 1 argument, got {}", args.len())));
    }
    Ok(alloc_string(args[0].to_string()))
}

/// `error(msg)` — raise a runtime error with `msg`. Always fails; never
/// returns. Stdlib code uses `return error("…")` to signal failure (e.g.
/// `result.unwrap` on an `Err`), and callers can intercept it with
/// `try … catch …`. Distinct from `result.err(v)`, which *constructs* an
/// error value rather than raising one.
pub(super) fn error_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let msg = if args.is_empty() {
        "error".to_string()
    } else {
        args[0].to_string()
    };
    Err(host.err(msg))
}

pub(super) fn read_line(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "read_line() takes no arguments, got {}",
            args.len()
        )));
    }
    if let Some(vm) = host.as_any_mut().downcast_mut::<VM>() {
        if let Some(seed) = vm.pop_stdin_line() {
            return Ok(alloc_string(seed));
        }
    }
    let mut s = String::new();
    let n = std::io::stdin()
        .read_line(&mut s)
        .map_err(|e| host.err(format!("read_line(): {e}")))?;
    if n == 0 {
        return Ok(Value::Nil);
    }
    // BUG FIX #15: Strip trailing '\n'/'\r\n' so comparisons like `if input == "yes"` work
    // as expected (same as Python/Ruby readline behavior).
    let s = s.trim_end_matches(['\n', '\r']).to_string();
    Ok(alloc_string(s))
}

pub(super) fn len(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("len() expects 1 argument, got {}", args.len())));
    }
    match &args[0] {
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => Ok(Value::Number(s.chars().count() as f64)),
            _ => Err(host.err("len() currently supports strings/arrays/maps only".into())),
        },
        Value::Array(a) => Ok(Value::Number(a.borrow().len() as f64)),
        Value::Map(m) => Ok(Value::Number(m.borrow().len() as f64)),
        _ => Err(host.err("len() currently supports strings/arrays/maps only".into())),
    }
}

pub(super) fn exec(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "exec", "exec"));
    }
    // SECURITY: exec(cmd_string) was vulnerable to shell injection because
    // the entire string was passed to `sh -c`. It is now deprecated in favour
    // of proc_spawn_wait(cmd, [args...]) which separates command from arguments.
    // Passing a single-token command (no spaces, no shell metacharacters) is
    // still allowed for backward compatibility, but any whitespace causes an error.
    if args.len() != 1 {
        return Err(host.err(format!("exec() expects 1 argument, got {}", args.len())));
    }
    let cmd = args[0]
        .as_str()
        .map_err(|m| host.err(format!("exec(): {m}")))?;
    let blocked: &[char] = &[
        ';', '&', '|', '$', '`', '>', '<', '!', '{', '}', '(', ')', '\'', '"', '\\', '*', '?', '[',
        ']', '#', '~',
    ];
    if cmd
        .chars()
        .any(|c| c.is_whitespace() || blocked.contains(&c))
    {
        return Err(host.err(
            "exec(): command contains shell metacharacters or whitespace — use proc_spawn_wait(cmd, args_array) instead.".into()
        ));
    }
    let status = std::process::Command::new(cmd)
        .status()
        .map_err(|e| host.err(format!("exec(): {e}")))?;
    Ok(Value::Number(status.code().unwrap_or(1) as f64))
}

pub(super) fn import(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("import() expects 1 argument, got {}", args.len())));
    }
    let path = args[0]
        .as_str()
        .map_err(|m| host.err(format!("import(): {m}")))?;
    host.import_module(path)
}

pub(super) fn gc_stats(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!("gc_stats() takes no arguments, got {}", args.len())));
    }
    Ok(alloc_string(format!(
        "string_allocs={} array_allocs={} map_allocs={} module_allocs={}",
        string_alloc_count(),
        ARRAY_ALLOCS.load(Ordering::Relaxed),
        MAP_ALLOCS.load(Ordering::Relaxed),
        MODULE_ALLOCS.load(Ordering::Relaxed)
    )))
}

/// assert(cond) or assert(cond, msg) — Rust-style debug assertion.
/// Throws RuntimeError if cond is falsy.
pub(super) fn assert_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(host.err("assert() requires at least 1 argument".into()));
    }
    let cond = &args[0];
    if !VM::is_truthy(cond) {
        let msg = if args.len() >= 2 {
            args[1].to_string()
        } else {
            "assertion failed".to_string()
        };
        return Err(host.err(format!("AssertionError: {msg}")));
    }
    Ok(Value::Nil)
}

pub(super) fn gc_collect(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "gc_collect() takes no arguments, got {}",
            args.len()
        )));
    }
    // BUG FIX #4: Previously this function forcefully cleared ALL open TCP/FFI/SQLite
    // handles, destroying any live connections. GC should only collect *unreachable*
    // resources. Native handle tables are intentionally NOT cleared here — their
    // entries are ref-counted by user-held handle numbers. Only the cycle-breaker
    // pass runs, which traces from GC roots and clears unreachable RC containers.
    let (arrays_cleared, maps_cleared, modules_cleared) = host.gc_collect_cycles();
    Ok(alloc_string(format!(
        "gc_collect: cycles arrays={} maps={} modules={}",
        arrays_cleared, maps_cleared, modules_cleared
    )))
}
