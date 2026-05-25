//! Atomic integer builtin implementations.
//!
//! Free functions backing the `atomic_*` builtins registered in
//! `VM::builtins()` (vm.rs). Each handle wraps a `VmSyncAtomic` (an
//! `AtomicI64`) shared across threads via the per-VM `handles`
//! registry; the builtins below are thin Rust ↔ pblang adapters for the
//! standard Java-`AtomicInteger`/Go-`atomic.Int64` shape.
//!
//!   - `atomic_new([initial])` → handle.
//!   - `atomic_get(h)` / `atomic_set(h, v)`.
//!   - `atomic_add(h, delta)` / `atomic_sub(h, delta)` — return the NEW
//!     value (post-update).
//!   - `atomic_cas(h, expected, new)` → bool (true on swap).
//!   - `atomic_swap(h, v)` → old value.
//!
//! All ordering is `SeqCst` for predictability across the pblang
//! threading model; if microbenchmarks ever demand it, weaker orderings
//! could be exposed as a separate API.
//!
//! Pure handle operations — no host capabilities. Always registered.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use pblang_ir::{RuntimeError, Value, VmHost};

use super::handles::{value_to_handle, vm_handles};
use crate::concurrency::VmSyncAtomic;

// ---- Atomic i64 (Java AtomicInteger / Go atomic.Int64 / Rust AtomicI64) ----

/// atomic_new(initial) — create an atomic integer.
pub(super) fn atomic_new_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let init = match args.first() {
        Some(Value::Number(n)) => *n as i64,
        None => 0,
        _ => return Err(host.err("atomic_new(val): expected number or no argument".into())),
    };
    let id = reg.atomic_next.fetch_add(1, Ordering::SeqCst);
    reg.atomics
        .lock()
        .map_err(|_| host.err("atomic_new: table".into()))?
        .insert(
            id,
            Arc::new(VmSyncAtomic {
                val: AtomicI64::new(init),
            }),
        );
    Ok(Value::Number(id as f64))
}

/// atomic_get(handle) — read the current value.
pub(super) fn atomic_get_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "atomic_get",
    )?;
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_get: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_get: invalid handle".into()))?;
    Ok(Value::Number(entry.val.load(Ordering::SeqCst) as f64))
}

/// atomic_set(handle, val) — store a value.
pub(super) fn atomic_set_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 2 {
        return Err(host.err("atomic_set(handle, val) expects 2 args".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "atomic_set")?;
    let val = match args[1] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_set: value must be a number".into())),
    };
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_set: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_set: invalid handle".into()))?;
    entry.val.store(val, Ordering::SeqCst);
    Ok(Value::Nil)
}

/// atomic_add(handle, delta) — atomically add delta, return NEW value.
pub(super) fn atomic_add_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 2 {
        return Err(host.err("atomic_add(handle, delta) expects 2 args".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "atomic_add")?;
    let delta = match args[1] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_add: delta must be a number".into())),
    };
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_add: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_add: invalid handle".into()))?;
    let new_val = entry.val.fetch_add(delta, Ordering::SeqCst) + delta;
    Ok(Value::Number(new_val as f64))
}

/// atomic_sub(handle, delta) — atomically subtract delta, return NEW value.
pub(super) fn atomic_sub_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 2 {
        return Err(host.err("atomic_sub(handle, delta) expects 2 args".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "atomic_sub")?;
    let delta = match args[1] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_sub: delta must be a number".into())),
    };
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_sub: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_sub: invalid handle".into()))?;
    let new_val = entry.val.fetch_sub(delta, Ordering::SeqCst) - delta;
    Ok(Value::Number(new_val as f64))
}

/// atomic_cas(handle, expected, new) — compare-and-swap.
/// Returns true if swap happened, false if current value != expected.
pub(super) fn atomic_cas_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 3 {
        return Err(host.err("atomic_cas(handle, expected, new) expects 3 args".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "atomic_cas")?;
    let expected = match args[1] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_cas: expected must be a number".into())),
    };
    let new_val = match args[2] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_cas: new_val must be a number".into())),
    };
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_cas: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_cas: invalid handle".into()))?;
    let ok = entry
        .val
        .compare_exchange(expected, new_val, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    Ok(Value::Bool(ok))
}

/// atomic_swap(handle, new_val) — atomically swap, return OLD value.
pub(super) fn atomic_swap_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 2 {
        return Err(host.err("atomic_swap(handle, val) expects 2 args".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "atomic_swap")?;
    let val = match args[1] {
        Value::Number(n) => n as i64,
        _ => return Err(host.err("atomic_swap: val must be a number".into())),
    };
    let t = reg
        .atomics
        .lock()
        .map_err(|_| host.err("atomic_swap: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("atomic_swap: invalid handle".into()))?;
    let old = entry.val.swap(val, Ordering::SeqCst);
    Ok(Value::Number(old as f64))
}
