//! Synchronization-primitive builtin implementations.
//!
//! Free functions backing the lock/barrier/once builtins registered in
//! `VM::builtins()` (vm.rs). Each handle wraps a per-VM
//! `concurrency::VmSync*` value stored in the `handles` registry:
//!
//!   - `mutex_new()` / `mutex_lock(h)` / `mutex_try_lock(h)` /
//!     `mutex_unlock(h)` — `Mutex<bool>` + `Condvar` (we hand-roll the
//!     locking state rather than holding `MutexGuard` across the FFI
//!     boundary because the guard isn't `Send`).
//!   - `rwlock_new()` / `rwlock_read(h)` / `rwlock_write(h)` /
//!     `rwlock_unlock(h)` — same shape but tracks reader count vs. writer.
//!   - `barrier_new(n)` / `barrier_wait(h)` — Go-style fixed-size barrier.
//!   - `once_new()` / `once_is_done(h)` — Go-style `sync.Once`. The
//!     "run my init fn exactly once" semantics live in `stdlib/sync.sapp`;
//!     this builtin pair just exposes the underlying done-flag.
//!
//! All operations are `SeqCst` for predictability under pblang's
//! VM-per-thread model.
//!
//! Pure handle operations — no host capabilities. Always registered.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pblang_ir::{RuntimeError, Value, VmHost};

use super::handles::{value_to_handle, vm_handles};
use crate::concurrency::{VmSyncBarrier, VmSyncMutex, VmSyncOnce, VmSyncRwLock};

// ---- Mutex builtins (Go sync.Mutex / Rust Mutex) ----

/// mutex_new() — create a mutex. Returns a handle.
pub(super) fn mutex_new_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = reg.mutex_next.fetch_add(1, Ordering::SeqCst);
    let m = Arc::new(VmSyncMutex {
        locked: Mutex::new(false),
        cv: std::sync::Condvar::new(),
    });
    reg.mutexes
        .lock()
        .map_err(|_| host.err("mutex_new: table unavailable".into()))?
        .insert(id, m);
    Ok(Value::Number(id as f64))
}

/// mutex_lock(handle) — acquire mutex; blocks until available.
pub(super) fn mutex_lock_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "mutex_lock",
    )?;
    let entry = {
        let t = reg
            .mutexes
            .lock()
            .map_err(|_| host.err("mutex_lock: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("mutex_lock: invalid handle".into()))?
    };
    let mut locked = entry
        .locked
        .lock()
        .map_err(|_| host.err("mutex_lock: lock".into()))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while *locked {
        if std::time::Instant::now() >= deadline {
            return Err(host.err("mutex_lock: deadlock timeout (30s)".into()));
        }
        let result = entry
            .cv
            .wait_timeout(locked, Duration::from_millis(10))
            .map_err(|_| host.err("mutex_lock: condvar".into()))?;
        locked = result.0;
    }
    *locked = true;
    Ok(Value::Nil)
}

/// mutex_try_lock(handle) — non-blocking acquire. Returns true if locked, false if busy.
pub(super) fn mutex_try_lock_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "mutex_try_lock",
    )?;
    let entry = {
        let t = reg
            .mutexes
            .lock()
            .map_err(|_| host.err("mutex_try_lock: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("mutex_try_lock: invalid handle".into()))?
    };
    let mut locked = entry
        .locked
        .lock()
        .map_err(|_| host.err("mutex_try_lock: lock".into()))?;
    if *locked {
        return Ok(Value::Bool(false));
    }
    *locked = true;
    Ok(Value::Bool(true))
}

/// mutex_unlock(handle) — release mutex.
pub(super) fn mutex_unlock_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "mutex_unlock",
    )?;
    let entry = {
        let t = reg
            .mutexes
            .lock()
            .map_err(|_| host.err("mutex_unlock: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("mutex_unlock: invalid handle".into()))?
    };
    let mut locked = entry
        .locked
        .lock()
        .map_err(|_| host.err("mutex_unlock: lock".into()))?;
    if !*locked {
        return Err(host.err("mutex_unlock: mutex is not locked".into()));
    }
    *locked = false;
    entry.cv.notify_one();
    Ok(Value::Nil)
}

// ---- RwLock builtins (sync.RWMutex in Go / Rust RwLock) ----

pub(super) fn rwlock_new_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = reg.rwlock_next.fetch_add(1, Ordering::SeqCst);
    reg.rwlocks
        .lock()
        .map_err(|_| host.err("rwlock_new: table".into()))?
        .insert(
            id,
            Arc::new(VmSyncRwLock {
                state: Mutex::new(0),
                cv: std::sync::Condvar::new(),
            }),
        );
    Ok(Value::Number(id as f64))
}

/// rwlock_read(handle) — acquire shared (read) lock.
pub(super) fn rwlock_read_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "rwlock_read",
    )?;
    let entry = {
        let t = reg
            .rwlocks
            .lock()
            .map_err(|_| host.err("rwlock_read: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("rwlock_read: invalid handle".into()))?
    };
    let mut state = entry
        .state
        .lock()
        .map_err(|_| host.err("rwlock_read: state".into()))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while *state < 0 {
        // write-locked
        if std::time::Instant::now() >= deadline {
            return Err(host.err("rwlock_read: timeout".into()));
        }
        let r = entry
            .cv
            .wait_timeout(state, Duration::from_millis(5))
            .map_err(|_| host.err("rwlock_read: condvar".into()))?;
        state = r.0;
    }
    *state += 1;
    Ok(Value::Nil)
}

/// rwlock_write(handle) — acquire exclusive (write) lock.
pub(super) fn rwlock_write_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "rwlock_write",
    )?;
    let entry = {
        let t = reg
            .rwlocks
            .lock()
            .map_err(|_| host.err("rwlock_write: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("rwlock_write: invalid handle".into()))?
    };
    let mut state = entry
        .state
        .lock()
        .map_err(|_| host.err("rwlock_write: state".into()))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while *state != 0 {
        // either read(s) or write in progress
        if std::time::Instant::now() >= deadline {
            return Err(host.err("rwlock_write: timeout".into()));
        }
        let r = entry
            .cv
            .wait_timeout(state, Duration::from_millis(5))
            .map_err(|_| host.err("rwlock_write: condvar".into()))?;
        state = r.0;
    }
    *state = -1; // write-locked
    Ok(Value::Nil)
}

/// rwlock_unlock(handle) — release read OR write lock.
pub(super) fn rwlock_unlock_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "rwlock_unlock",
    )?;
    let entry = {
        let t = reg
            .rwlocks
            .lock()
            .map_err(|_| host.err("rwlock_unlock: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("rwlock_unlock: invalid handle".into()))?
    };
    let mut state = entry
        .state
        .lock()
        .map_err(|_| host.err("rwlock_unlock: state".into()))?;
    if *state == -1 {
        *state = 0;
    }
    // write unlock
    else if *state > 0 {
        *state -= 1;
    }
    // read unlock
    else {
        return Err(host.err("rwlock_unlock: lock not held".into()));
    }
    entry.cv.notify_all();
    Ok(Value::Nil)
}

// ---- Barrier (CyclicBarrier in Java / sync.WaitGroup N threads) ----

/// barrier_new(n) — create a barrier that releases all n threads when all have arrived.
pub(super) fn barrier_new_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let n = match args.first() {
        Some(Value::Number(x)) if *x >= 1.0 => *x as usize,
        _ => return Err(host.err("barrier_new(n): expected positive integer n".into())),
    };
    let id = reg.barrier_next.fetch_add(1, Ordering::SeqCst);
    reg.barriers
        .lock()
        .map_err(|_| host.err("barrier_new: table".into()))?
        .insert(
            id,
            Arc::new(VmSyncBarrier {
                n,
                arrived: Mutex::new(0),
                cv: std::sync::Condvar::new(),
            }),
        );
    Ok(Value::Number(id as f64))
}

/// barrier_wait(handle) — wait at barrier until all n threads have arrived.
pub(super) fn barrier_wait_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "barrier_wait",
    )?;
    let entry = {
        let t = reg
            .barriers
            .lock()
            .map_err(|_| host.err("barrier_wait: table".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("barrier_wait: invalid handle".into()))?
    };
    let mut arrived = entry
        .arrived
        .lock()
        .map_err(|_| host.err("barrier_wait: lock".into()))?;
    *arrived += 1;
    if *arrived >= entry.n {
        *arrived = 0; // reset for reuse (CyclicBarrier semantics)
        entry.cv.notify_all();
        return Ok(Value::Bool(true)); // was the releasing thread
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let r = entry
            .cv
            .wait_timeout(arrived, Duration::from_millis(5))
            .map_err(|_| host.err("barrier_wait: condvar".into()))?;
        arrived = r.0;
        if *arrived == 0 {
            return Ok(Value::Bool(false));
        } // released, not the last
        if std::time::Instant::now() >= deadline {
            return Err(host.err("barrier_wait: timeout (60s) — possible deadlock".into()));
        }
    }
}

// ---- Once (sync.Once in Go / lazy by-name in Kotlin) ----

/// once_new() — create a Once handle.
pub(super) fn once_new_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = reg.once_next.fetch_add(1, Ordering::SeqCst);
    reg.once_flags
        .lock()
        .map_err(|_| host.err("once_new: table".into()))?
        .insert(
            id,
            Arc::new(VmSyncOnce {
                done: AtomicBool::new(false),
                running: Mutex::new(false),
                cv: std::sync::Condvar::new(),
            }),
        );
    Ok(Value::Number(id as f64))
}

/// once_is_done(handle) — true if the once has already run.
pub(super) fn once_is_done_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "once_is_done",
    )?;
    let t = reg
        .once_flags
        .lock()
        .map_err(|_| host.err("once_is_done: table".into()))?;
    let entry = t
        .get(&id)
        .ok_or_else(|| host.err("once_is_done: invalid handle".into()))?;
    Ok(Value::Bool(entry.done.load(Ordering::SeqCst)))
}
