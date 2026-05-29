//! Type predicate / introspection / freeze builtin implementations.
//!
//! Free functions backing:
//!
//!   - `type_of(v)` → string ("number"/"bool"/"nil"/"string"/"fn"/
//!     "closure"/"builtin"/"module"/"array"/"map").
//!   - `is_number` / `is_string` / `is_array` / `is_map` / `is_bool` /
//!     `is_fn` / `is_nil` — one-liner type tests.
//!   - `type_assert(v, type_str)` — error on mismatch, returns nil.
//!   - `require_type(v, type_str)` — error on mismatch, returns v.
//!   - `freeze(v)` — sets `__frozen: true` on a map (other types pass through).
//!   - `is_frozen(v)` — `true` if the map has `__frozen: true`.
//!
//! `type_assert` / `require_type` both compare against the same string
//! set that `type_of` produces (so e.g. `type_assert(f, "fn")` works).
//!
//! Pure computation — no host capabilities. Always registered.

use pblang_ir::{alloc_string, Obj, RuntimeError, Value, VmHost};

pub(super) fn type_of(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("type() expects 1 argument, got {}", args.len())));
    }
    let s = match &args[0] {
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Nil => "nil",
        Value::Obj(o) => match o.as_ref() {
            Obj::String(_) => "string",
            Obj::BigInt(_) => "bigint",
            Obj::Rational(_) => "rational",
            Obj::Decimal(_) => "decimal",
        },
        Value::Function(_) => "fn",
        Value::Closure(_) => "closure",
        Value::Builtin(_) => "builtin",
        Value::Module(_) => "module",
        Value::Array(_) => "array",
        Value::Map(_) => "map",
    };
    Ok(alloc_string(s.to_string()))
}

/// type_assert(value, type_str) — assert that value has the expected type.
/// type_str: "number", "string", "bool", "nil", "array", "map", "closure", "fn".
pub(super) fn type_assert(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("type_assert(value, type_str) requires 2 arguments".into()));
    }
    let expected = args[1]
        .as_str()
        .map_err(|_| host.err("type_assert: second argument must be a string".to_string()))?;
    // BUG FIX #13: Must match type_of() exactly so type_assert(f, "fn") works.
    let actual = type_name(&args[0]);
    if actual != expected {
        return Err(host.err(format!("TypeError: expected {expected}, got {actual}")));
    }
    Ok(Value::Nil)
}

/// require_type(val, type_str) — like type_assert but returns the value on success.
pub(super) fn require_type(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err("require_type(val, type_str) requires 2 arguments".into()));
    }
    let expected = args[1]
        .as_str()
        .map_err(|_| host.err("require_type: second argument must be a string".to_string()))?;
    let actual = type_name(&args[0]);
    if actual != expected {
        return Err(host.err(format!(
            "TypeError: require_type expected {expected}, got {actual}"
        )));
    }
    Ok(args[0].clone())
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Nil => "nil",
        Value::Obj(o) => match o.as_ref() {
            Obj::String(_) => "string",
            Obj::BigInt(_) => "bigint",
            Obj::Rational(_) => "rational",
            Obj::Decimal(_) => "decimal",
        },
        Value::Array(_) => "array",
        Value::Map(_) => "map",
        Value::Function(_) => "fn",
        Value::Closure(_) => "closure",
        Value::Builtin(_) => "builtin",
        Value::Module(_) => "module",
    }
}

/// freeze(val) — mark a map/array as frozen (immutable).
/// Returns the value with __frozen: true inserted.
/// Frozen maps reject attribute assignment at runtime.
pub(super) fn freeze(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err("freeze(val) takes 1 argument".into()));
    }
    match &args[0] {
        Value::Map(m) => {
            m.borrow_mut()
                .insert("__frozen".to_string(), Value::Bool(true));
            Ok(args[0].clone())
        }
        _ => Ok(args[0].clone()), // non-maps pass through
    }
}

/// is_frozen(val) — returns true if val has __frozen: true.
pub(super) fn is_frozen(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err("is_frozen(val) takes 1 argument".into()));
    }
    match &args[0] {
        Value::Map(m) => {
            let frozen = m.borrow().get("__frozen").cloned();
            Ok(Value::Bool(matches!(frozen, Some(Value::Bool(true)))))
        }
        _ => Ok(Value::Bool(false)),
    }
}

// -------------------------------------------------------------------------
// Single-discriminant predicates
// -------------------------------------------------------------------------

pub(super) fn is_number(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Number(_)))))
}
pub(super) fn is_string(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Obj(_)))))
}
pub(super) fn is_array(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Array(_)))))
}
pub(super) fn is_map(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Map(_)))))
}
pub(super) fn is_bool(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Bool(_)))))
}
pub(super) fn is_fn(_h: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(
        args.first(),
        Some(Value::Function(_) | Value::Closure(_) | Value::Builtin(_))
    )))
}
pub(super) fn is_nil(_host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    Ok(Value::Bool(matches!(args.first(), Some(Value::Nil) | None)))
}
