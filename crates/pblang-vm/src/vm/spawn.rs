//! Spawn / channel concurrency builtin implementations.
//!
//! Free functions backing the spawn + channel + ticker + timeout builtins
//! registered in `VM::builtins()` (vm.rs). This is the most VM-coupled
//! cluster — it threads VM stacks, snapshots open upvalues, and starts
//! per-spawn `tokio`-free `std::thread`s that each construct a fresh `VM`.
//!
//!   - **Spawning**: `spawn(closure)` → numeric handle; `spawn_n(n, factory)`
//!     → array<handle>; `join(handle)` → result value (blocks); `join_all([h…])`
//!     → array<result> (waits for all in order, ignores individual errors).
//!   - **Channels**: `channel()` (unbounded) / `channel_buffered(cap)`;
//!     `send(ch, v)` / `recv(ch)` / `recv_timeout(ch, ms)`;
//!     `channel_close(ch)` / `channel_is_closed(ch)`; `channel_size(ch)`.
//!   - **Select**: `select(channels)` returns `{ index, value }` (blocking);
//!     `select_with_timeout(channels, ms)` adds a deadline.
//!   - **Timers**: `ticker(period_ms)` → channel that fires periodically;
//!     `timeout_channel(ms)` → channel that fires once after `ms`.
//!
//! Open upvalues are snapshotted from the parent VM stack at spawn time;
//! closed upvalues transfer directly. Module globals best-effort-transfer
//! via the wire format from `vm/spawn_wire.rs` (non-transferable values
//! like open Tcp/FFI/SQLite handles are dropped from the child's globals
//! with a silent skip — channels are the cross-thread comms primitive).
//!
//! `spawn` is gated on `host.allow_spawn()`. Channels are pure handle
//! operations; not gated.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use pblang_ir::chunk::{Chunk, OpCode, Span};
use pblang_ir::{alloc_string, Module, RuntimeError, Upvalue, Value, VmHost};

use serde_json as sj;

use super::handles::{value_to_handle, vm_handles};
use super::spawn_wire::{
    spawn_closure_from_wire, spawn_function_to_wire, spawn_value_from_json, spawn_value_from_wire,
    spawn_value_to_json, spawn_value_to_wire,
};
use super::{alloc_array, alloc_map, StdlibConfig, VmLimits, VM};
use crate::concurrency::{SpawnWireClosure, SpawnWireValue, VmChannelBuf};

/// Stack size for spawned VM threads. Plain `thread::spawn` gives ~2 MiB, far less
/// than the main thread's ~8 MiB, so a spawned closure running deeply-recursive
/// pblang (e.g. the self-hosted compiler's parser/semantic walk) overflows and aborts
/// the whole process. We give spawned threads a generous default and let it be tuned
/// via `PBLANG_SPAWN_STACK_MB` for unusually deep workloads.
fn spawn_stack_size() -> usize {
    const DEFAULT_MB: usize = 32;
    let mb = std::env::var("PBLANG_SPAWN_STACK_MB")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&m| m > 0)
        .unwrap_or(DEFAULT_MB);
    mb.saturating_mul(1024 * 1024)
}

pub(super) fn spawn_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_spawn() {
        return Err(host.err("spawn not allowed".into()));
    }
    if args.len() != 1 {
        return Err(host.err(format!("spawn() expects 1 argument, got {}", args.len())));
    }
    let closure = match &args[0] {
        Value::Closure(c) => c.clone(),
        _ => return Err(host.err("spawn: expected a closure (function with no arguments)".into())),
    };
    // Serialize closure captures. Closed upvalues transfer directly.
    // Open upvalues are snapshotted from the current VM stack at spawn time.
    let mut upvalues_wire: Vec<SpawnWireValue> = Vec::new();
    for uv in &closure.upvalues {
        let val = match &*uv.borrow() {
            Upvalue::Closed(v) => v.clone(),
            Upvalue::Open(i) => {
                let vm = match host.as_any_mut().downcast_mut::<VM>() {
                    Some(vm) => vm,
                    None => {
                        return Err(
                            host.err("spawn: host does not support open-upvalue snapshot".into())
                        )
                    }
                };
                match vm.stack.get(*i).cloned() {
                    Some(v) => v,
                    None => return Err(host.err("spawn: open upvalue points outside stack".into())),
                }
            }
        };
        upvalues_wire.push(spawn_value_to_wire(&val).map_err(|e| host.err(e))?);
    }
    let root_wire = SpawnWireClosure {
        function: spawn_function_to_wire(&closure.function).map_err(|e| host.err(e))?,
        upvalues: upvalues_wire,
    };
    let module_globals_wire: HashMap<String, SpawnWireValue> = {
        let globals = closure.module.borrow();
        let mut out = HashMap::new();
        for (k, v) in globals.globals.iter() {
            // Best-effort transfer: keep transferable globals only.
            if let Ok(wv) = spawn_value_to_wire(v) {
                out.insert(k.clone(), wv);
            }
        }
        out
    };
    let spawn_limits = host.spawn_limits();

    let handles = vm_handles(host)?;
    let id = handles.spawn_next.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::channel();
    handles
        .spawn
        .lock()
        .map_err(|_| host.err("internal: spawn table unavailable".into()))?
        .insert(id, rx);

    let child_handles = Arc::clone(&handles);
    let config = StdlibConfig::default();
    let spawn_result = thread::Builder::new()
        .name("pblang-spawn".into())
        .stack_size(spawn_stack_size())
        .spawn(move || {
        let module = Rc::new(RefCell::new(Module {
            globals: HashMap::new(),
            parent: None,
            exports: HashMap::new(),
        }));
        let mut vm = VM::with_stdlib(config);
        vm.handle_registry = child_handles;
        if let Some(lim) = spawn_limits {
            vm.set_limits(Some(VmLimits {
                max_instructions: lim.max_instructions,
                max_stack_depth: lim.max_stack_depth,
                max_memory_bytes: lim.max_memory_bytes,
                io_timeout_ms: lim.io_timeout_ms,
                // Child inherits parent's cap by default.
                max_external_json_bytes: None,
            }));
        }
        // Connect the child closure's module to the VM's builtins
        // so that builtin globals (recv, send, etc.) are accessible.
        {
            let builtins_parent = vm.module.borrow().parent.clone();
            module.borrow_mut().parent = builtins_parent;
        }
        // Hydrate transferred globals *before* taking the mutable
        // borrow on `module`, because a global may itself be an
        // imported Module whose rehydration needs to shared-borrow
        // `module` to read its `parent` (the builtins chain). Holding
        // a borrow_mut across spawn_value_from_wire would clash and
        // panic at runtime.
        let mut globals_hydrated: Vec<(String, Value)> =
            Vec::with_capacity(module_globals_wire.len());
        for (k, wv) in &module_globals_wire {
            if let Ok(v) = spawn_value_from_wire(wv, module.clone()) {
                globals_hydrated.push((k.clone(), v));
            }
        }
        {
            let mut gm = module.borrow_mut();
            for (k, v) in globals_hydrated {
                gm.globals.insert(k, v);
            }
        }
        let child_closure = match spawn_closure_from_wire(&root_wire, module) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(e));
                return;
            }
        };
        let wrapper = Chunk {
            instructions: vec![OpCode::OpConstant(0), OpCode::OpCall(0), OpCode::OpReturn],
            constants: vec![Value::Closure(child_closure)],
            spans: vec![Span::default(), Span::default(), Span::default()],
            source_name: None,
            source: None,
        };
        let result = match vm.run_return_value(wrapper) {
            Ok(v) => spawn_value_to_wire(&v),
            Err(e) => Err(e.message),
        };
        let _ = tx.send(result);
    });
    // If the OS could not create the thread, the child's `tx` was dropped with the
    // closure, so a later join() observes a closed channel and surfaces a spawn
    // failure — no panic, graceful degradation.
    if let Err(e) = spawn_result {
        return Err(host.err(format!("spawn: could not create thread: {e}")));
    }
    Ok(Value::Number(id as f64))
}

pub(super) fn join_builtin(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() != 1 {
        return Err(host.err(format!("join() expects 1 argument, got {}", args.len())));
    }
    let id = value_to_handle(host, args[0].clone(), "join")?;
    let rx = {
        let mut t = reg
            .spawn
            .lock()
            .map_err(|_| host.err("internal: spawn table unavailable".into()))?;
        t.remove(&id)
            .ok_or_else(|| host.err("join: invalid or already joined handle".into()))?
    };
    let result = rx
        .recv()
        .map_err(|_| host.err("join: channel closed (spawn failed?)".into()))?;
    match result {
        Ok(v) => {
            let module = host
                .as_any_mut()
                .downcast_mut::<VM>()
                .map(|vm| vm.module.clone())
                .unwrap_or_else(|| Rc::new(RefCell::new(Module::default())));
            spawn_value_from_wire(&v, module).map_err(|e| host.err(e))
        }
        Err(msg) => Err(host.err(msg)),
    }
}

// ---- Channel builtins ----

pub(super) fn channel_builtin(
    host: &mut dyn VmHost,
    _args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
    let cb = VmChannelBuf {
        buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        closed: Arc::new(AtomicBool::new(false)),
        capacity: 0, // unbounded
        space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
        item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
    };
    reg.channels
        .lock()
        .map_err(|_| host.err("channel: table unavailable".into()))?
        .insert(id, cb);
    let mut m = HashMap::new();
    m.insert("sender".into(), Value::Number(id as f64));
    m.insert("receiver".into(), Value::Number(id as f64));
    Ok(alloc_map(m))
}

/// channel_buffered(capacity) — Go-style buffered channel.
/// Sends don't block until the buffer is full; recv blocks when empty.
pub(super) fn channel_buffered_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let capacity = match args.first() {
        Some(Value::Number(n)) if *n >= 1.0 => *n as usize,
        _ => {
            return Err(
                host.err("channel_buffered(capacity): expected positive integer capacity".into())
            )
        }
    };
    let id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
    let cb = VmChannelBuf {
        buf: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(
            capacity,
        ))),
        closed: Arc::new(AtomicBool::new(false)),
        capacity,
        space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
        item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
    };
    reg.channels
        .lock()
        .map_err(|_| host.err("channel_buffered: table unavailable".into()))?
        .insert(id, cb);
    let mut m = HashMap::new();
    m.insert("sender".into(), Value::Number(id as f64));
    m.insert("receiver".into(), Value::Number(id as f64));
    m.insert("capacity".into(), Value::Number(capacity as f64));
    Ok(alloc_map(m))
}

pub(super) fn channel_send_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() != 2 {
        return Err(host.err(format!("send() expects 2 arguments, got {}", args.len())));
    }
    let id = value_to_handle(host, args[0].clone(), "send")?;
    let json = spawn_value_to_json(&args[1]).map_err(|e| host.err(e))?;
    let json_str = sj::to_string(&json).map_err(|e| host.err(e.to_string()))?;
    let cb = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("send: table unavailable".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("send: invalid channel id".into()))?
    };
    if cb.closed.load(Ordering::SeqCst) {
        return Err(host.err("send: channel is closed".into()));
    }
    // For bounded channels: block until there is space
    if cb.capacity > 0 {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            {
                let buf_len = cb
                    .buf
                    .lock()
                    .map_err(|_| host.err("send: buf lock unavailable".into()))?
                    .len();
                if buf_len < cb.capacity {
                    break; // space available
                }
            }
            if cb.closed.load(Ordering::SeqCst) {
                return Err(host.err("send: channel closed while waiting for space".into()));
            }
            if std::time::Instant::now() >= deadline {
                return Err(host.err("send: timeout waiting for channel space".into()));
            }
            // Wait for space signal with 5ms sleep (condvar across threads is tricky here)
            let (lock, cv) = cb.space_cv.as_ref();
            let guard = lock
                .lock()
                .map_err(|_| host.err("send: condvar lock".into()))?;
            let _ = cv.wait_timeout(guard, Duration::from_millis(5));
        }
    }
    cb.buf
        .lock()
        .map_err(|_| host.err("send: buf lock unavailable".into()))?
        .push_back(json_str);
    // Notify waiting receivers
    cb.item_cv.1.notify_all();
    Ok(Value::Nil)
}

pub(super) fn channel_recv_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    channel_recv_impl(host, args, 30_000)
}

/// recv_timeout(receiver, timeout_ms) — recv with explicit timeout.
/// Returns nil if the channel closed OR timed out; use channel_is_closed() to distinguish.
pub(super) fn channel_recv_timeout_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("recv_timeout(receiver, ms) expects 2 arguments".into()));
    }
    let ms = match args[1] {
        Value::Number(n) if n >= 0.0 => n as u64,
        _ => return Err(host.err("recv_timeout: timeout_ms must be a non-negative number".into())),
    };
    channel_recv_impl(host, vec![args[0].clone()], ms)
}

pub(super) fn channel_recv_impl(
    host: &mut dyn VmHost,
    args: Vec<Value>,
    timeout_ms: u64,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.is_empty() {
        return Err(host.err("recv() expects 1 argument, got 0".into()));
    }
    let id = value_to_handle(host, args[0].clone(), "recv")?;
    let cb = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("recv: table unavailable".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("recv: invalid channel id".into()))?
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        // Try to dequeue
        {
            let mut buf = cb
                .buf
                .lock()
                .map_err(|_| host.err("recv: buf lock unavailable".into()))?;
            if let Some(json_str) = buf.pop_front() {
                // Notify blocked senders that space is available
                cb.space_cv.1.notify_all();
                let j: sj::Value = sj::from_str(&json_str).map_err(|e| host.err(e.to_string()))?;
                return Ok(spawn_value_from_json(&j));
            }
        }
        if cb.closed.load(Ordering::SeqCst) {
            return Ok(Value::Nil); // channel closed + empty
        }
        if std::time::Instant::now() >= deadline {
            return Ok(Value::Nil); // timeout
        }
        // Sleep briefly then retry (condvar across threads requires Arc<Condvar>)
        let (lock, cv) = cb.item_cv.as_ref();
        let guard = lock
            .lock()
            .map_err(|_| host.err("recv: condvar lock".into()))?;
        let _ = cv.wait_timeout(guard, Duration::from_millis(1));
    }
}

pub(super) fn channel_close_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() != 1 {
        return Err(host.err(format!(
            "channel_close() expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "channel_close")?;
    let cb = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("channel_close: table unavailable".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("channel_close: invalid channel id".into()))?
    };
    cb.closed.store(true, Ordering::SeqCst);
    // Wake all blocked receivers so they can return nil
    cb.item_cv.1.notify_all();
    // Wake all blocked senders so they can error out
    cb.space_cv.1.notify_all();
    Ok(Value::Nil)
}

pub(super) fn channel_is_closed_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() != 1 {
        return Err(host.err(format!(
            "channel_is_closed() expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "channel_is_closed")?;
    let cb = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("channel_is_closed: table unavailable".into()))?;
        t.get(&id)
            .cloned()
            .ok_or_else(|| host.err("channel_is_closed: invalid channel id".into()))?
    };
    Ok(Value::Bool(cb.closed.load(Ordering::SeqCst)))
}

pub(super) fn channel_select_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() != 1 {
        return Err(host.err(format!("select() expects 1 argument, got {}", args.len())));
    }
    let ids: Vec<usize> = match &args[0] {
        Value::Array(a) => {
            let a = a.borrow();
            let mut out = Vec::with_capacity(a.len());
            for v in a.iter() {
                out.push(value_to_handle(host, v.clone(), "select")?);
            }
            out
        }
        _ => return Err(host.err("select(): expected array of channel ids".into())),
    };
    // Snapshot VmChannelBuf handles upfront (VmChannelBuf is Clone + Arc-backed, cheap).
    let cbs: Vec<Option<VmChannelBuf>> = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("select: table unavailable".into()))?;
        ids.iter().map(|id| t.get(id).cloned()).collect()
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let mut all_closed_empty = true;
        for (i, cb_opt) in cbs.iter().enumerate() {
            if let Some(cb) = cb_opt {
                let is_closed = cb.closed.load(Ordering::SeqCst);
                if let Ok(mut buf) = cb.buf.try_lock() {
                    if !buf.is_empty() {
                        // Dequeue the value and return {index, value}
                        let json_str = buf.pop_front().unwrap_or_default();
                        let v = sj::from_str::<sj::Value>(&json_str)
                            .map(|j| spawn_value_from_json(&j))
                            .unwrap_or(Value::Nil);
                        let mut m = std::collections::HashMap::new();
                        m.insert("index".into(), Value::Number(i as f64));
                        m.insert("value".into(), v);
                        return Ok(alloc_map(m));
                    }
                    // channel has data or could still receive
                    if !is_closed {
                        all_closed_empty = false;
                    }
                } else {
                    all_closed_empty = false; // locked = possibly active
                }
            }
        }
        // If all channels are closed and empty, return nil
        if all_closed_empty {
            return Ok(Value::Nil);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(Value::Nil); // timeout → nil (not an error)
        }
        thread::sleep(Duration::from_millis(1));
    }
}

// ---- Enhanced channel/threading builtins ----

/// select_with_timeout(receivers_array, timeout_ms)
/// Like select() but returns nil after timeout_ms milliseconds instead of nil on closed.
/// Returns {index, value, timed_out: bool} or nil if all closed.
pub(super) fn channel_select_timeout_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    if args.len() < 2 {
        return Err(host.err("select_with_timeout(channels, ms) expects 2 arguments".into()));
    }
    let ms = match args[1] {
        Value::Number(n) if n >= 0.0 => n as u64,
        _ => return Err(host.err("select_with_timeout: ms must be non-negative number".into())),
    };
    let ids: Vec<usize> = match &args[0] {
        Value::Array(a) => {
            let a = a.borrow();
            let mut out = Vec::with_capacity(a.len());
            for v in a.iter() {
                out.push(value_to_handle(host, v.clone(), "select_with_timeout")?);
            }
            out
        }
        _ => return Err(host.err("select_with_timeout: first arg must be array".into())),
    };
    let cbs: Vec<Option<VmChannelBuf>> = {
        let t = reg
            .channels
            .lock()
            .map_err(|_| host.err("select_with_timeout: table".into()))?;
        ids.iter().map(|id| t.get(id).cloned()).collect()
    };
    let deadline = std::time::Instant::now() + Duration::from_millis(ms);
    loop {
        let mut all_closed_empty = true;
        for (i, cb_opt) in cbs.iter().enumerate() {
            if let Some(cb) = cb_opt {
                let is_closed = cb.closed.load(Ordering::SeqCst);
                if let Ok(mut buf) = cb.buf.try_lock() {
                    if !buf.is_empty() {
                        let json_str = buf.pop_front().unwrap_or_default();
                        cb.space_cv.1.notify_all();
                        let v = sj::from_str::<sj::Value>(&json_str)
                            .map(|j| spawn_value_from_json(&j))
                            .unwrap_or(Value::Nil);
                        let mut m = std::collections::HashMap::new();
                        m.insert("index".into(), Value::Number(i as f64));
                        m.insert("value".into(), v);
                        m.insert("timed_out".into(), Value::Bool(false));
                        return Ok(alloc_map(m));
                    }
                    if !is_closed {
                        all_closed_empty = false;
                    }
                } else {
                    all_closed_empty = false;
                }
            }
        }
        if all_closed_empty {
            return Ok(Value::Nil);
        }
        if std::time::Instant::now() >= deadline {
            let mut m = std::collections::HashMap::new();
            m.insert("timed_out".into(), Value::Bool(true));
            return Ok(alloc_map(m));
        }
        thread::sleep(Duration::from_millis(1));
    }
}

/// channel_size(receiver) — number of items currently buffered in the channel.
pub(super) fn channel_size_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let id = value_to_handle(
        host,
        args.into_iter().next().unwrap_or(Value::Nil),
        "channel_size",
    )?;
    let t = reg
        .channels
        .lock()
        .map_err(|_| host.err("channel_size: table".into()))?;
    let cb = t
        .get(&id)
        .ok_or_else(|| host.err("channel_size: invalid channel id".into()))?;
    let n = cb
        .buf
        .lock()
        .map_err(|_| host.err("channel_size: buf lock".into()))?
        .len();
    Ok(Value::Number(n as f64))
}

/// join_all(handles_array) — join all thread handles, return array of results.
/// Like Go's WaitGroup or Java's CompletableFuture.allOf().
pub(super) fn join_all_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let handles = match args.first() {
        Some(Value::Array(a)) => a.borrow().clone(),
        _ => return Err(host.err("join_all(handles): expected array of handles".into())),
    };
    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        let id = value_to_handle(host, h, "join_all")?;
        let rx = {
            let mut t = reg
                .spawn
                .lock()
                .map_err(|_| host.err("join_all: spawn table unavailable".into()))?;
            t.remove(&id)
                .ok_or_else(|| host.err("join_all: invalid or already joined handle".into()))?
        };
        let result = rx
            .recv()
            .map_err(|_| host.err("join_all: channel closed".into()))?;
        let module = host
            .as_any_mut()
            .downcast_mut::<VM>()
            .map(|vm| vm.module.clone())
            .unwrap_or_else(|| Rc::new(RefCell::new(Module::default())));
        let val = match result {
            Ok(v) => spawn_value_from_wire(&v, module).map_err(|e| host.err(e))?,
            Err(msg) => {
                // Propagate error as {ok:false, error:msg}
                let mut m = std::collections::HashMap::new();
                m.insert("ok".to_string(), Value::Bool(false));
                m.insert("error".to_string(), alloc_string(msg));
                alloc_map(m)
            }
        };
        results.push(val);
    }
    Ok(alloc_array(results))
}

/// spawn_n(n, fn) — spawn n threads each running fn(), return array of handles.
/// Like Go's fan-out pattern: for i := 0; i < n; i++ { go fn() }.
pub(super) fn spawn_n_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("spawn_n(n, fn) expects 2 arguments".into()));
    }
    let n = match args[0] {
        Value::Number(x) if x >= 1.0 => x as usize,
        _ => return Err(host.err("spawn_n: n must be a positive integer".into())),
    };
    let fn_val = args[1].clone();
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let h = spawn_builtin(host, vec![fn_val.clone()])?;
        handles.push(h);
    }
    Ok(alloc_array(handles))
}

/// ticker(interval_ms) — create a channel that receives a tick every interval_ms.
/// Like Go's time.NewTicker(). Close the channel to stop it.
/// Returns the receiver handle (a channel).
pub(super) fn ticker_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let ms = match args.first() {
        Some(Value::Number(n)) if *n >= 1.0 => *n as u64,
        _ => return Err(host.err("ticker(interval_ms): expected positive ms".into())),
    };
    // Create an unbounded channel for tick events
    let tick_ch_id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
    let tick_cb = VmChannelBuf {
        buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        closed: Arc::new(AtomicBool::new(false)),
        capacity: 0,
        space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
        item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
    };
    reg.channels
        .lock()
        .map_err(|_| host.err("ticker: table unavailable".into()))?
        .insert(tick_ch_id, tick_cb.clone());
    // Spawn background ticker thread
    thread::spawn(move || {
        let mut tick_count = 0u64;
        loop {
            thread::sleep(Duration::from_millis(ms));
            if tick_cb.closed.load(Ordering::SeqCst) {
                break;
            }
            tick_count += 1;
            let json_str = format!("{}", tick_count);
            if let Ok(mut buf) = tick_cb.buf.lock() {
                buf.push_back(json_str);
            }
            tick_cb.item_cv.1.notify_all();
        }
    });
    // Return {sender, receiver} for compatibility
    let mut m = std::collections::HashMap::new();
    m.insert("sender".into(), Value::Number(tick_ch_id as f64));
    m.insert("receiver".into(), Value::Number(tick_ch_id as f64));
    Ok(alloc_map(m))
}

/// timeout_channel(ms) — returns a channel that sends exactly once after ms, then closes.
/// Like Go's time.After(). Use with select to implement timeouts.
pub(super) fn timeout_channel_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    let reg = vm_handles(host)?;
    let ms = match args.first() {
        Some(Value::Number(n)) if *n >= 0.0 => *n as u64,
        _ => return Err(host.err("timeout_channel(ms): expected non-negative ms".into())),
    };
    let ch_id = reg.channel_next.fetch_add(1, Ordering::SeqCst);
    let cb = VmChannelBuf {
        buf: Arc::new(Mutex::new(std::collections::VecDeque::new())),
        closed: Arc::new(AtomicBool::new(false)),
        capacity: 1, // buffered(1) so the fire-and-forget doesn't block
        space_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
        item_cv: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
    };
    reg.channels
        .lock()
        .map_err(|_| host.err("timeout_channel: table unavailable".into()))?
        .insert(ch_id, cb.clone());
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ms));
        if !cb.closed.load(Ordering::SeqCst) {
            if let Ok(mut buf) = cb.buf.lock() {
                buf.push_back("true".to_string());
            }
            cb.item_cv.1.notify_all();
            cb.closed.store(true, Ordering::SeqCst);
        }
    });
    let mut m = std::collections::HashMap::new();
    m.insert("sender".into(), Value::Number(ch_id as f64));
    m.insert("receiver".into(), Value::Number(ch_id as f64));
    Ok(alloc_map(m))
}
