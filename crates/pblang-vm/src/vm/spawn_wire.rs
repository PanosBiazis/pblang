//! Serialisation helpers for `spawn` / `join` value transfer between VMs.
//!
//! Each pblang thread runs in its own VM with its own GC arena (the Erlang /
//! Go isolation model). Values can only cross a thread boundary if they are
//! **transferable** — primitives, strings, deeply-cloneable arrays / maps,
//! and the closed-upvalue closures we lower here. This module converts a
//! [`Value`] into a [`SpawnWireValue`] (and back) so the channel layer can
//! ship it without sharing an `Rc`.
//!
//! These functions were previously inline in `vm.rs`; split out so the VM
//! file shrinks toward something readable. Public visibility is restricted
//! to the parent module (`pub(super)`).
//
// Reads `alloc_array` / `alloc_map` from the parent module (`super`); those
// register the new container with the GC registries so cycle collection sees
// them, which is why we can't just call `Rc::new(RefCell::new(...))` here.

use crate::concurrency::{SpawnWireClosure, SpawnWireFunction, SpawnWireValue};
use pblang_ir::chunk::Chunk;
use pblang_ir::{alloc_string, Closure, Function, Module, Obj, Upvalue, Value};
use serde_json as sj;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::{alloc_array, alloc_map};

/// Serialize a Value into `serde_json::Value` (used for cross-VM channel
/// payloads that need a stable on-wire shape — e.g. handles passed to
/// host-side queues).
pub(super) fn spawn_value_to_json(v: &Value) -> Result<sj::Value, String> {
    match v {
        Value::Nil => Ok(sj::Value::Null),
        Value::Bool(b) => Ok(sj::Value::Bool(*b)),
        Value::Number(n) => Ok(sj::Number::from_f64(*n)
            .map(sj::Value::Number)
            .unwrap_or(sj::Value::Null)),
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => Ok(sj::Value::String(s.clone())),
            // Cross-thread transfer has no bigint/rational wire tag yet; send the
            // exact decimal / "n/d" as a string (receiver sees a string).
            Obj::BigInt(n) => Ok(sj::Value::String(n.to_string())),
            Obj::Rational(r) => Ok(sj::Value::String(r.to_string())),
            Obj::Decimal(d) => Ok(sj::Value::String(d.to_string())),
        },
        Value::Array(a) => {
            let a = a.borrow();
            let mut out = Vec::with_capacity(a.len());
            for x in a.iter() {
                out.push(spawn_value_to_json(x)?);
            }
            Ok(sj::Value::Array(out))
        }
        Value::Map(m) => {
            let m = m.borrow();
            let mut out = sj::Map::new();
            for (k, vv) in m.iter() {
                out.insert(k.clone(), spawn_value_to_json(vv)?);
            }
            Ok(sj::Value::Object(out))
        }
        _ => Err(
            "spawn: value not transferable (use only numbers, bools, strings, arrays, maps)".into(),
        ),
    }
}

/// Deserialize JSON to Value (used in spawn child thread).
pub(super) fn spawn_value_from_json(v: &sj::Value) -> Value {
    match v {
        sj::Value::Null => Value::Nil,
        sj::Value::Bool(b) => Value::Bool(*b),
        sj::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        sj::Value::String(s) => alloc_string(s.clone()),
        sj::Value::Array(items) => alloc_array(items.iter().map(spawn_value_from_json).collect()),
        sj::Value::Object(map) => {
            let mut out = HashMap::new();
            for (k, vv) in map.iter() {
                out.insert(k.clone(), spawn_value_from_json(vv));
            }
            alloc_map(out)
        }
    }
}

pub(super) fn spawn_function_to_wire(function: &Rc<Function>) -> Result<SpawnWireFunction, String> {
    let defaults = function
        .defaults
        .iter()
        .map(spawn_value_to_wire)
        .collect::<Result<Vec<_>, _>>()?;
    let constants = function
        .chunk
        .constants
        .iter()
        .map(spawn_value_to_wire)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SpawnWireFunction {
        name: function.name.clone(),
        arity: function.arity,
        min_arity: function.min_arity,
        defaults,
        is_vararg: function.is_vararg,
        instructions: function.chunk.instructions.clone(),
        spans: function.chunk.spans.clone(),
        constants,
        param_types: function.param_types.clone(),
    })
}

pub(super) fn spawn_closure_to_wire(closure: &Rc<Closure>) -> Result<SpawnWireClosure, String> {
    let function = spawn_function_to_wire(&closure.function)?;
    let mut upvalues = Vec::with_capacity(closure.upvalues.len());
    for uv in &closure.upvalues {
        let v = match &*uv.borrow() {
            Upvalue::Closed(v) => v.clone(),
            Upvalue::Open(_) => {
                return Err(
                    "spawn: closure transfer requires closed upvalues (open upvalue found)".into(),
                );
            }
        };
        upvalues.push(spawn_value_to_wire(&v)?);
    }
    Ok(SpawnWireClosure { function, upvalues })
}

/// Serialize a Value to a send-safe representation for spawn/join.
pub(super) fn spawn_value_to_wire(v: &Value) -> Result<SpawnWireValue, String> {
    match v {
        Value::Nil => Ok(SpawnWireValue::Nil),
        Value::Bool(b) => Ok(SpawnWireValue::Bool(*b)),
        Value::Number(n) => Ok(SpawnWireValue::Number(*n)),
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => Ok(SpawnWireValue::String(s.clone())),
            Obj::BigInt(n) => Ok(SpawnWireValue::String(n.to_string())),
            Obj::Rational(r) => Ok(SpawnWireValue::String(r.to_string())),
            Obj::Decimal(d) => Ok(SpawnWireValue::String(d.to_string())),
        },
        Value::Array(a) => {
            let a = a.borrow();
            let mut out = Vec::with_capacity(a.len());
            for x in a.iter() {
                out.push(spawn_value_to_wire(x)?);
            }
            Ok(SpawnWireValue::Array(out))
        }
        Value::Map(m) => {
            let m = m.borrow();
            let mut out = HashMap::with_capacity(m.len());
            for (k, vv) in m.iter() {
                out.insert(k.clone(), spawn_value_to_wire(vv)?);
            }
            Ok(SpawnWireValue::Map(out))
        }
        Value::Function(f) => Ok(SpawnWireValue::Function(Box::new(spawn_function_to_wire(
            f,
        )?))),
        Value::Closure(c) => Ok(SpawnWireValue::Closure(Box::new(spawn_closure_to_wire(c)?))),
        Value::Module(m) => {
            // Snapshot the module so the child can call `m.foo()` from
            // a spawned closure. Transferable entries are copied; the
            // rest (builtins, opaque resource handles) are dropped on
            // a best-effort basis. Without this, a closure that
            // captures `let thread = import(...)` silently loses the
            // `thread` upvalue when crossing the spawn boundary.
            let m = m.borrow();
            let mut globals = HashMap::new();
            for (k, v) in m.globals.iter() {
                if let Ok(wv) = spawn_value_to_wire(v) {
                    globals.insert(k.clone(), wv);
                }
            }
            let mut exports = HashMap::new();
            for (k, v) in m.exports.iter() {
                if let Ok(wv) = spawn_value_to_wire(v) {
                    exports.insert(k.clone(), wv);
                }
            }
            Ok(SpawnWireValue::Module { globals, exports })
        }
        _ => Err(
            "spawn: value not transferable (unsupported: builtin/resource handle)".into(),
        ),
    }
}

pub(super) fn spawn_function_from_wire(
    f: &SpawnWireFunction,
    module: Rc<RefCell<Module>>,
) -> Result<Rc<Function>, String> {
    let defaults = f
        .defaults
        .iter()
        .map(|v| spawn_value_from_wire(v, module.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let constants = f
        .constants
        .iter()
        .map(|v| spawn_value_from_wire(v, module.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Rc::new(Function {
        name: f.name.clone(),
        arity: f.arity,
        min_arity: f.min_arity,
        defaults,
        is_vararg: f.is_vararg,
        chunk: Chunk {
            instructions: f.instructions.clone(),
            spans: f.spans.clone(),
            constants,
            source_name: None,
            source: None,
        },
        param_types: f.param_types.clone(),
    }))
}

pub(super) fn spawn_closure_from_wire(
    c: &SpawnWireClosure,
    module: Rc<RefCell<Module>>,
) -> Result<Rc<Closure>, String> {
    let function = spawn_function_from_wire(&c.function, module.clone())?;
    let mut upvalues = Vec::with_capacity(c.upvalues.len());
    for v in &c.upvalues {
        let vv = spawn_value_from_wire(v, module.clone())?;
        upvalues.push(Rc::new(RefCell::new(Upvalue::Closed(vv))));
    }
    Ok(Rc::new(Closure {
        function,
        upvalues,
        module,
    }))
}

pub(super) fn spawn_value_from_wire(
    v: &SpawnWireValue,
    module: Rc<RefCell<Module>>,
) -> Result<Value, String> {
    match v {
        SpawnWireValue::Nil => Ok(Value::Nil),
        SpawnWireValue::Bool(b) => Ok(Value::Bool(*b)),
        SpawnWireValue::Number(n) => Ok(Value::Number(*n)),
        SpawnWireValue::String(s) => Ok(alloc_string(s.clone())),
        SpawnWireValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(spawn_value_from_wire(item, module.clone())?);
            }
            Ok(alloc_array(out))
        }
        SpawnWireValue::Map(map) => {
            let mut out = HashMap::with_capacity(map.len());
            for (k, vv) in map {
                out.insert(k.clone(), spawn_value_from_wire(vv, module.clone())?);
            }
            Ok(alloc_map(out))
        }
        SpawnWireValue::Function(f) => Ok(Value::Function(spawn_function_from_wire(f, module)?)),
        SpawnWireValue::Closure(c) => Ok(Value::Closure(spawn_closure_from_wire(c, module)?)),
        SpawnWireValue::Module {
            globals: g_wire,
            exports: e_wire,
        } => {
            // Rehydrate the snapshot into a fresh Module in the child
            // VM. We *must* set the new module's `parent` to the same
            // builtins chain that the child's root module uses —
            // otherwise the imported module's closures (which call
            // `atomic_sub`, `channel_*`, `send`/`recv`, etc.) will fail
            // global lookups in the child. The root module's `parent`
            // was wired up to the VM's builtins by the spawn driver
            // (see vm/spawn.rs around line 141); we piggy-back on it.
            let builtins_parent = module.borrow().parent.clone();
            let new_mod = Rc::new(RefCell::new(Module {
                globals: HashMap::new(),
                parent: builtins_parent,
                exports: HashMap::new(),
            }));
            // Recursively hydrate the contained values into a Vec
            // *before* taking the mutable borrow on `new_mod`, because
            // a nested entry might itself be a Module that needs to
            // read `new_mod.borrow().parent` (a shared borrow) — that
            // would deadlock against an outer `new_mod.borrow_mut()`.
            let mut globals_hydrated: Vec<(String, Value)> = Vec::with_capacity(g_wire.len());
            for (k, vv) in g_wire {
                globals_hydrated.push((k.clone(), spawn_value_from_wire(vv, new_mod.clone())?));
            }
            let mut exports_hydrated: Vec<(String, Value)> = Vec::with_capacity(e_wire.len());
            for (k, vv) in e_wire {
                exports_hydrated.push((k.clone(), spawn_value_from_wire(vv, new_mod.clone())?));
            }
            {
                let mut m = new_mod.borrow_mut();
                for (k, v) in globals_hydrated {
                    m.globals.insert(k, v);
                }
                for (k, v) in exports_hydrated {
                    m.exports.insert(k, v);
                }
            }
            Ok(Value::Module(new_mod))
        }
    }
}
