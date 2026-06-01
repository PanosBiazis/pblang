//! Map builtin implementations.
//!
//! Free functions backing the map-flavoured builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `map_keys(m)` — returns a sorted array of string keys.
//!   - `map_has(m, k)` — bool.
//!   - `map_remove(m, k)` — in-place delete; returns the removed value or
//!     nil.
//!
//! Pure computation — no host capabilities. Always registered.
//
// Visibility: every item is `pub(super)` so vm.rs reaches them via
// `fun: maps::map_keys` in the BuiltinFn table.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_array;

pub(super) fn map_keys(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("map_keys() expects 1 argument, got {}", args.len())));
    }
    let m = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(host.err("map_keys(): expected map".into())),
    };
    let mut keys = m
        .borrow()
        .keys()
        .map(|k| alloc_string(k.clone()))
        .collect::<Vec<_>>();
    keys.sort_by_key(|a| a.to_string());
    Ok(alloc_array(keys))
}

pub(super) fn map_has(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("map_has() expects 2 arguments, got {}", args.len())));
    }
    let m = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(host.err("map_has(): expected map".into())),
    };
    let k = args[1]
        .as_str()
        .map_err(|mm| host.err(format!("map_has(): {mm}")))?;
    let has = { m.borrow().contains_key(k) };
    Ok(Value::Bool(has))
}

/// `has_member(obj, name)` — structural-conformance reflection primitive that
/// backs the `interface` keyword (and is useful directly). Returns `true` if
/// `obj` is a map that has `name` as a key, OR has it on its `__class` table
/// chain (so native-`class` instances — whose methods live on the class, not the
/// instance — and `oop` inheritance both conform). Total over all value types:
/// non-maps return `false` rather than erroring, so it composes inside `if` /
/// `assert` guards. The `__class` walk is depth-bounded (32) to tolerate cycles.
pub(super) fn has_member(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "has_member() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let name = args[1]
        .as_str()
        .map_err(|mm| host.err(format!("has_member(): {mm}")))?
        .to_string();
    let mut cur = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Ok(Value::Bool(false)),
    };
    for _ in 0..32 {
        let (found, parent) = {
            let b = cur.borrow();
            let found = b.contains_key(name.as_str());
            let parent = match b.get("__class") {
                Some(Value::Map(p)) => Some(p.clone()),
                _ => None,
            };
            (found, parent)
        };
        if found {
            return Ok(Value::Bool(true));
        }
        match parent {
            Some(p) => cur = p,
            None => break,
        }
    }
    Ok(Value::Bool(false))
}

pub(super) fn map_remove(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "map_remove() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let m = match &args[0] {
        Value::Map(m) => m.clone(),
        _ => return Err(host.err("map_remove(): expected map".into())),
    };
    let k = args[1]
        .as_str()
        .map_err(|mm| host.err(format!("map_remove(): {mm}")))?;
    let v = m.borrow_mut().remove(k).unwrap_or(Value::Nil);
    Ok(v)
}
