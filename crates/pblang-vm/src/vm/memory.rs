//! Runtime memory-safety / ownership / borrowing builtin implementations.
//!
//! Free functions backing the memory-handle builtins registered in
//! `VM::builtins()` (vm.rs). All operations route through
//! `VmHost::mem_*`, which lives in the per-VM `MemorySafetyHost`
//! tracker; the builtins below are thin Rust ↔ pblang adapters.
//!
//!   - **Allocate / free** — `allocate(value[, mutable])` returns a
//!     numeric handle; `deallocate(h)` releases it. The `memory_alloc`
//!     and `memory_free` aliases exist for the
//!     `stdlib/memory.sapp` API name.
//!   - **Read / write** — `mem_load(h)` / `mem_store(h, v)`, plus the
//!     `memory_get` / `memory_set` aliases.
//!   - **Borrow / release** — `borrow_immutable(h)` / `borrow_mutable(h)`
//!     / `borrow_unique(h)` / `release_borrow(h)`. These track Rust-style
//!     borrow-count invariants at runtime.
//!   - **Lifetime** — `push_lifetime()` / `pop_lifetime()` mark a scope
//!     boundary; values allocated between the push and the matching pop
//!     are freed when the scope exits.
//!   - **Diagnostics** — `memory_stats()` returns a map of live
//!     allocations + borrow counters.
//!
//! Pure handle operations — no host capabilities. Always registered.

use pblang_ir::{RuntimeError, Value, VmHost};

// ---- Runtime memory safety (ownership / borrowing) ----

pub(super) fn allocate_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(host.err("allocate(value, is_mutable?) expects at least 1 arg".into()));
    }
    let value = args[0].clone();
    let is_mutable = match args.get(1) {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => *n != 0.0,
        _ => {
            return Err(host.err("allocate: is_mutable must be bool or number".into()));
        }
    };
    let h = host.mem_allocate(value, is_mutable)?;
    Ok(Value::Number(h))
}

pub(super) fn borrow_immutable_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("borrow_immutable(handle) expects a number".into())),
    };
    Ok(Value::Number(host.mem_borrow_immutable(handle)?))
}

pub(super) fn borrow_mutable_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("borrow_mutable(handle) expects a number".into())),
    };
    Ok(Value::Number(host.mem_borrow_mutable(handle)?))
}

pub(super) fn borrow_unique_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("borrow_unique(handle) expects a number".into())),
    };
    Ok(Value::Number(host.mem_borrow_unique(handle)?))
}

pub(super) fn release_borrow_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(
            host.err("release_borrow(borrow_id) or release_borrow(handle, type)".into())
        );
    }
    if args.len() == 1 {
        let borrow_id = match &args[0] {
            Value::Number(n) => *n,
            _ => return Err(host.err("release_borrow: expected number".into())),
        };
        host.mem_release_borrow_id(borrow_id)?;
    } else {
        let handle = match &args[0] {
            Value::Number(n) => *n,
            _ => return Err(host.err("release_borrow: handle must be a number".into())),
        };
        let ty = match &args[1] {
            Value::Number(n) => *n as u8,
            _ => return Err(host.err("release_borrow: type must be 0, 1, or 2".into())),
        };
        host.mem_release_borrow_type(handle, ty)?;
    }
    Ok(Value::Nil)
}

pub(super) fn mem_load_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("load(handle) expects a number".into())),
    };
    host.mem_load(handle)
}

pub(super) fn mem_store_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("store(handle, value) expects 2 args".into()));
    }
    let handle = match &args[0] {
        Value::Number(n) => *n,
        _ => return Err(host.err("store: handle must be a number".into())),
    };
    host.mem_store(handle, args[1].clone())?;
    Ok(Value::Nil)
}

pub(super) fn deallocate_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("deallocate(handle) expects a number".into())),
    };
    host.mem_deallocate(handle)?;
    Ok(Value::Nil)
}

pub(super) fn push_lifetime_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    Ok(Value::Number(host.mem_push_lifetime()?))
}

pub(super) fn pop_lifetime_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    Ok(Value::Number(host.mem_pop_lifetime()?))
}

pub(super) fn memory_stats_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    host.mem_stats()
}

// ---- User-facing memory safety builtins (memory_ prefix) ----

/// memory_alloc(value, is_mutable?) → memory_id
pub(super) fn memory_alloc_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    allocate_builtin(host, args)
}

/// memory_borrow(memory_id, type) → borrow_id
/// type: 0 = immutable, 1 = mutable
pub(super) fn memory_borrow_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("memory_borrow(handle, type) expects 2 args".into()));
    }
    let handle = match &args[0] {
        Value::Number(n) => *n,
        _ => return Err(host.err("memory_borrow: handle must be a number".into())),
    };
    let borrow_type = match &args[1] {
        Value::Number(n) => *n as i64,
        _ => {
            return Err(
                host.err("memory_borrow: type must be 0 (immutable) or 1 (mutable)".into())
            )
        }
    };
    match borrow_type {
        0 => Ok(Value::Number(host.mem_borrow_immutable(handle)?)),
        1 => Ok(Value::Number(host.mem_borrow_mutable(handle)?)),
        _ => {
            Err(host.err("memory_borrow: type must be 0 (immutable) or 1 (mutable)".into()))
        }
    }
}

/// memory_release(memory_id, borrow_id?) → nil
/// Release a borrow by borrow_id, or release first borrow on memory_id.
pub(super) fn memory_release_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(host.err("memory_release(borrow_id) or memory_release(handle, type) expects at least 1 arg".into()));
    }
    if args.len() == 1 {
        // Release by borrow_id
        let borrow_id = match &args[0] {
            Value::Number(n) => *n,
            _ => return Err(host.err("memory_release: borrow_id must be a number".into())),
        };
        host.mem_release_borrow_id(borrow_id)?;
    } else {
        // Release by handle + type
        let handle = match &args[0] {
            Value::Number(n) => *n,
            _ => return Err(host.err("memory_release: handle must be a number".into())),
        };
        let ty = match &args[1] {
            Value::Number(n) => *n as u8,
            _ => return Err(host.err("memory_release: type must be 0, 1, or 2".into())),
        };
        host.mem_release_borrow_type(handle, ty)?;
    }
    Ok(Value::Nil)
}

/// memory_get(memory_id) → value
pub(super) fn memory_get_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("memory_get(handle) expects a number".into())),
    };
    host.mem_load(handle)
}

/// memory_set(memory_id, value) → nil
pub(super) fn memory_set_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("memory_set(handle, value) expects 2 args".into()));
    }
    let handle = match &args[0] {
        Value::Number(n) => *n,
        _ => return Err(host.err("memory_set: handle must be a number".into())),
    };
    host.mem_store(handle, args[1].clone())?;
    Ok(Value::Nil)
}

/// memory_free(memory_id) → nil
pub(super) fn memory_free_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let handle = match args.first() {
        Some(Value::Number(n)) => *n,
        _ => return Err(host.err("memory_free(handle) expects a number".into())),
    };
    host.mem_deallocate(handle)?;
    Ok(Value::Nil)
}
