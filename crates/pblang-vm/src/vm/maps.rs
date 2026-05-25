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
