//! OS / signal builtin implementations.
//!
//! Free functions backing the OS-flavoured builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `os_args()` — `std::env::args().skip(1)` as an array of strings.
//!   - `os_exit(code)` — `std::process::exit(code)`.
//!   - `os_signal_trap(name, handler)` — install a real OS signal
//!     handler via `crate::signals::install`. Stashes the handler in the
//!     per-VM `signal_handlers` registry (keyed by canonical signal
//!     name); pblang code calls `os_signal_poll()` to drain pending
//!     signals and dispatch handlers explicitly (async-signal-safety
//!     means we cannot call .sapp code directly from the libc handler).
//!   - `os_signal_pending(name)` — drained count for a single signal.
//!   - `os_signal_poll()` — map<name, pending_count> for every name
//!     with a handler installed.
//!   - `os_signal_handler_get(name)` — the handler `fn`/`nil` for a name.
//!   - `os_which(cmd)` — safely find an executable in PATH (no shell;
//!     rejects path separators and shell metacharacters in `cmd`).
//!     Gated on `host.allow_exec()`.

use std::collections::HashMap;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::errors::vm_cap;
use super::{alloc_array, alloc_map, VM};

pub(super) fn os_args_builtin(
    _host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let argv: Vec<Value> = std::env::args().skip(1).map(alloc_string).collect();
    Ok(alloc_array(argv))
}

pub(super) fn os_exit_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("os_exit() expects 1 argument, got {}", args.len())));
    }
    let code = match args[0] {
        Value::Number(n) => n as i32,
        _ => return Err(host.err("os_exit(): expected number".into())),
    };
    std::process::exit(code);
}

pub(super) fn os_signal_trap_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // os_signal_trap(name, handler_or_nil) -> map{ ok, signal, signum }
    //
    // Installs a real OS-level signal handler that increments a process-global
    // pending counter. The handler closure (if provided) is stored in the
    // per-VM signal registry and returned by os_signal_handler_get(name);
    // pblang programs call os_signal_poll() from regular code to drain
    // pending signals and dispatch handlers explicitly (async-signal-safety
    // means we cannot call .sapp code directly from the libc handler).
    if args.len() != 2 {
        return Err(host.err(format!(
            "os_signal_trap() expects 2 arguments (name, handler), got {}",
            args.len()
        )));
    }
    let name = args[0]
        .as_str()
        .map_err(|m| host.err(format!("os_signal_trap(): {m}")))?
        .to_string();
    let handler = args[1].clone();
    let signum = crate::signals::signal_number(&name).map_err(|e| host.err(e))?;
    crate::signals::install(signum).map_err(|e| host.err(e))?;
    // Stash handler in the VM-local registry, keyed by canonical signal name.
    if let Some(vm) = host.as_any_mut().downcast_mut::<VM>() {
        let key = name.trim().to_ascii_uppercase();
        let key = if let Some(rest) = key.strip_prefix("SIG") {
            format!("SIG{rest}")
        } else {
            format!("SIG{key}")
        };
        vm.signal_handlers.borrow_mut().insert(key, handler);
    }
    let mut m = HashMap::new();
    m.insert("ok".into(), Value::Bool(true));
    m.insert("signal".into(), alloc_string(name));
    m.insert("signum".into(), Value::Number(signum as f64));
    Ok(alloc_map(m))
}

pub(super) fn os_signal_pending_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // os_signal_pending(name) -> number (drains pending count for name)
    if args.len() != 1 {
        return Err(host.err(format!(
            "os_signal_pending() expects 1 argument (name), got {}",
            args.len()
        )));
    }
    let name = args[0]
        .as_str()
        .map_err(|m| host.err(format!("os_signal_pending(): {m}")))?
        .to_string();
    let signum = crate::signals::signal_number(&name).map_err(|e| host.err(e))?;
    let count = crate::signals::take_pending(signum);
    Ok(Value::Number(count as f64))
}

pub(super) fn os_signal_poll_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // os_signal_poll() -> map{ name -> pending_count } for every name with handler installed.
    if !args.is_empty() {
        return Err(host.err("os_signal_poll() takes no arguments".into()));
    }
    let names: Vec<String> = match host.as_any_mut().downcast_mut::<VM>() {
        Some(vm) => vm.signal_handlers.borrow().keys().cloned().collect(),
        None => vec![],
    };
    let mut out = HashMap::new();
    for n in names {
        if let Ok(num) = crate::signals::signal_number(&n) {
            let c = crate::signals::take_pending(num);
            if c > 0 {
                out.insert(n, Value::Number(c as f64));
            }
        }
    }
    Ok(alloc_map(out))
}

pub(super) fn os_signal_handler_get_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    // os_signal_handler_get(name) -> fn | nil
    if args.len() != 1 {
        return Err(host.err(format!(
            "os_signal_handler_get() expects 1 argument (name), got {}",
            args.len()
        )));
    }
    let name = args[0]
        .as_str()
        .map_err(|m| host.err(format!("os_signal_handler_get(): {m}")))?
        .to_string();
    let key = name.trim().to_ascii_uppercase();
    let key = if let Some(rest) = key.strip_prefix("SIG") {
        format!("SIG{rest}")
    } else {
        format!("SIG{key}")
    };
    if let Some(vm) = host.as_any_mut().downcast_mut::<VM>() {
        if let Some(v) = vm.signal_handlers.borrow().get(&key) {
            return Ok(v.clone());
        }
    }
    Ok(Value::Nil)
}

/// os_which(cmd) — safely find an executable in PATH.
/// Uses std::process::Command to check each PATH entry directly (no shell injection).
pub(super) fn os_which_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(vm_cap!(host, "os_which", "exec"));
    }
    if args.len() != 1 {
        return Err(host.err(format!("os_which() expects 1 argument, got {}", args.len())));
    }
    let cmd = args[0]
        .as_str()
        .map_err(|m| host.err(format!("os_which(): {m}")))?;
    // Validate the command name (no path separators, no metacharacters)
    let is_dangerous: bool = cmd.chars().any(|c| "'/\\;&| \t\n><`$(){}!#~\"".contains(c));
    if is_dangerous || cmd.is_empty() {
        return Err(host.err(format!("os_which(): unsafe command name: {}", cmd)));
    }
    // Search PATH entries directly (no shell)
    let path_var = std::env::var("PATH").unwrap_or_default();
    let separator = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.split(separator) {
        let candidate = std::path::Path::new(dir).join(cmd);
        if candidate.is_file() {
            if let Some(s) = candidate.to_str() {
                return Ok(alloc_string(s.to_string()));
            }
        }
        // Windows: also try with .exe extension
        #[cfg(windows)]
        {
            let candidate_exe = std::path::Path::new(dir).join(format!("{}.exe", cmd));
            if candidate_exe.is_file() {
                if let Some(s) = candidate_exe.to_str() {
                    return Ok(alloc_string(s.to_string()));
                }
            }
        }
    }
    Ok(Value::Nil)
}
