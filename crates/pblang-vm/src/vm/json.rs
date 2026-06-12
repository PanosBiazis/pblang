//! JSON builtin implementations.
//!
//! Free functions backing the JSON-flavoured builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `json_parse(s)` → pblang Value (Nil / Bool / Number / String /
//!     Array / Map). Uses `serde_json::from_str` and surfaces line/column
//!     in error messages.
//!   - `json_stringify(v)` → JSON string. Closures, fns, builtins, and
//!     modules return an error.
//!
//! `json_to_value` / `value_to_json` are the bridging helpers used by
//! these builtins **and** by sibling categories that need to round-trip
//! VM values through JSON (e.g. `http_response_json`, `ai_chat`,
//! `polyglot_*` envelopes). Kept `pub(super)` for those.
//!
//! Pure computation — no host capabilities. Always registered.

use std::collections::HashMap;

use pblang_ir::{alloc_string, Obj, RuntimeError, Value, VmHost};
use serde_json as sj;

use super::{alloc_array, alloc_map};

/// Convert a serde_json value into a pblang VM value.
pub(super) fn json_to_value(v: &sj::Value) -> Value {
    match v {
        sj::Value::Null => Value::Nil,
        sj::Value::Bool(b) => Value::Bool(*b),
        sj::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
        sj::Value::String(s) => alloc_string(s.clone()),
        sj::Value::Array(items) => {
            let out = items.iter().map(json_to_value).collect::<Vec<_>>();
            alloc_array(out)
        }
        sj::Value::Object(map) => {
            let mut out = HashMap::<String, Value>::new();
            for (k, vv) in map.iter() {
                out.insert(k.clone(), json_to_value(vv));
            }
            alloc_map(out)
        }
    }
}

/// Convert a pblang VM value into a serde_json value.
///
/// Returns Err for `Function`/`Closure`/`Builtin`/`Module` — those have
/// no JSON representation.
pub(super) fn value_to_json(v: &Value) -> Result<sj::Value, String> {
    Ok(match v {
        Value::Nil => sj::Value::Null,
        Value::Bool(b) => sj::Value::Bool(*b),
        Value::Number(n) => sj::Number::from_f64(*n)
            .map(sj::Value::Number)
            .unwrap_or(sj::Value::Null),
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => sj::Value::String(s.clone()),
            // JSON numbers can't hold arbitrary precision, so emit a bigint as
            // its exact decimal string and a rational as its "n/d" string.
            Obj::BigInt(n) => sj::Value::String(n.to_string()),
            Obj::Rational(r) => sj::Value::String(r.to_string()),
            Obj::Decimal(d) => sj::Value::String(d.to_string()),
        },
        Value::Array(a) => {
            let a = a.borrow();
            sj::Value::Array(
                a.iter()
                    .map(|x| value_to_json(x).unwrap_or(sj::Value::Null))
                    .collect(),
            )
        }
        Value::Map(m) => {
            let m = m.borrow();
            let mut out = sj::Map::new();
            for (k, vv) in m.iter() {
                out.insert(k.clone(), value_to_json(vv).unwrap_or(sj::Value::Null));
            }
            sj::Value::Object(out)
        }
        _ => return Err("json_stringify: unsupported value type".into()),
    })
}

pub(super) fn json_parse(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "json_parse() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("json_parse(): {m}")))?;
    let v: sj::Value = sj::from_str(s).map_err(|e| {
        host.err(format!(
            "json_parse(): {} at line {} column {}",
            e,
            e.line(),
            e.column()
        ))
    })?;
    Ok(json_to_value(&v))
}

pub(super) fn json_stringify(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "json_stringify() expects 1 argument, got {}",
            args.len()
        )));
    }
    let j = value_to_json(&args[0]).map_err(|m| host.err(m))?;
    let s = sj::to_string(&j).map_err(|e| host.err(format!("json_stringify(): {e}")))?;
    Ok(alloc_string(s))
}
