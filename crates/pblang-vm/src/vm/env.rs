//! Environment / platform builtin implementations.
//!
//! Free functions backing:
//!
//!   - `env_get(name)` → string | nil. Read-only, gated on the `env` capability
//!     (default on; disabled by `--sandbox`/`--deny-env`) so a sandbox can't
//!     exfiltrate secrets such as `OPENAI_API_KEY`.
//!   - `env_set(name, value)` — **permanently disabled**. `std::env::set_var`
//!     is unsound in multi-threaded programs (undefined behaviour when
//!     called concurrently with env reads on Linux), and pblang supports
//!     VM-per-thread spawning. Returns an error; callers should pass
//!     values via channel arguments or per-VM state instead.
//!   - `platform()` → string. `std::env::consts::OS`
//!     (`"linux"`/`"macos"`/`"windows"`/...).
//!
//! `env_set` is gated on `host.allow_exec()` for the rejection path's
//! consistency with the rest of the env/exec surface.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

pub(super) fn env_get(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    // env_get reads environment variables. Read-only, but a sandboxed script with
    // net/http but no env capability could otherwise exfiltrate secrets like
    // OPENAI_API_KEY/AWS_* — so it is gated on the `env` capability (default on).
    if !host.allow_env() {
        return Err(host.err("env_get(): env access denied".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!("env_get() expects 1 argument, got {}", args.len())));
    }
    let k = args[0]
        .as_str()
        .map_err(|m| host.err(format!("env_get(): {m}")))?;
    match std::env::var(k) {
        Ok(v) => Ok(alloc_string(v)),
        Err(_) => Ok(Value::Nil),
    }
}

pub(super) fn env_set(host: &mut dyn VmHost, _args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_exec() {
        return Err(host.err("exec/env access denied".into()));
    }
    // SECURITY: std::env::set_var is unsound in multi-threaded programs (undefined
    // behaviour when called concurrently with env reads on Linux). Since pblang
    // supports VM-per-thread spawning, this builtin is permanently disabled.
    // Use os.sapp's env utilities or pass values via channel arguments instead.
    Err(host.err(
        "env_set() is disabled: mutating the process environment is not thread-safe.                  Pass values via channel arguments or use per-VM state.".into()
    ))
}

pub(super) fn platform(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err("platform() takes no arguments".into()));
    }
    Ok(alloc_string(std::env::consts::OS.to_string()))
}
