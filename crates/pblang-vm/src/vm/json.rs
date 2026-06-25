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

/// Recursion-depth cap for value→JSON serialization.
///
/// pblang arrays and maps are `Rc<RefCell<…>>`, so user code can build a
/// reference cycle (`let a = []; push(a, a)`). Serializing one would recurse
/// until the native stack overflowed and *aborted the whole process* —
/// uncatchable from pblang `try`/`catch`. Capping the depth turns that into an
/// ordinary, catchable error. The cap is far deeper than any legitimate JSON
/// value (serde_json's own parse limit is 128), so it only trips on cycles or
/// pathologically deep data. Mirrors `spawn_wire::SPAWN_WIRE_MAX_DEPTH`.
const MAX_JSON_DEPTH: usize = 1024;

/// Convert a pblang VM value into a serde_json value.
///
/// Returns Err for `Function`/`Closure`/`Builtin`/`Module` — those have
/// no JSON representation — and for values that exceed [`MAX_JSON_DEPTH`]
/// (deeply nested or cyclic), which would otherwise overflow the stack.
pub(super) fn value_to_json(v: &Value) -> Result<sj::Value, String> {
    value_to_json_depth(v, 0)
}

fn value_to_json_depth(v: &Value, depth: usize) -> Result<sj::Value, String> {
    if depth > MAX_JSON_DEPTH {
        return Err("json_stringify: value too deeply nested or cyclic to serialize".into());
    }
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
            let mut out = Vec::with_capacity(a.len());
            for x in a.iter() {
                out.push(value_to_json_depth(x, depth + 1)?);
            }
            sj::Value::Array(out)
        }
        Value::Map(m) => {
            let m = m.borrow();
            let mut out = sj::Map::new();
            for (k, vv) in m.iter() {
                out.insert(k.clone(), value_to_json_depth(vv, depth + 1)?);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_to_json_rejects_cyclic_array_without_overflow() {
        // pblang arrays are Rc<RefCell<Vec<Value>>>, so user code can build a
        // self-referential cycle (`let a = []; push(a, a)`). Serializing one used
        // to recurse until the native stack overflowed and *aborted the whole
        // process* — uncatchable from pblang. It must instead be an ordinary Err.
        let a = alloc_array(vec![]);
        if let Value::Array(ref rc) = a {
            rc.borrow_mut().push(a.clone());
        } else {
            panic!("alloc_array did not return an Array");
        }
        let err = value_to_json(&a).expect_err("cyclic value must error, not overflow");
        assert!(
            err.contains("nested") || err.contains("cyclic"),
            "expected a depth/cycle error, got: {err}"
        );
    }

    #[test]
    fn value_to_json_rejects_cyclic_map_without_overflow() {
        // Same hazard via a map that contains itself under a key.
        let m = alloc_map(HashMap::new());
        if let Value::Map(ref rc) = m {
            rc.borrow_mut().insert("self".to_string(), m.clone());
        } else {
            panic!("alloc_map did not return a Map");
        }
        let err = value_to_json(&m).expect_err("cyclic map must error, not overflow");
        assert!(
            err.contains("nested") || err.contains("cyclic"),
            "expected a depth/cycle error, got: {err}"
        );
    }

    #[test]
    fn value_to_json_rejects_pathologically_deep_nesting() {
        // A linear-but-very-deep structure (no cycle) must also be rejected
        // rather than overflow — the depth cap is the backstop for both.
        // Depth comfortably past MAX_JSON_DEPTH (1024) so the cap trips, but
        // shallow enough that the test's own recursive Drop of the structure
        // doesn't overflow (that is a separate, far-deeper limit).
        let mut v = Value::Number(1.0);
        for _ in 0..4_000 {
            v = alloc_array(vec![v]);
        }
        assert!(
            value_to_json(&v).is_err(),
            "pathologically deep nesting must error, not overflow"
        );
    }

    #[test]
    fn value_to_json_still_serializes_normal_nesting() {
        // Ordinary nested data (well within the cap) must serialize fine.
        let inner = alloc_array(vec![Value::Number(1.0), Value::Number(2.0)]);
        let mut m = HashMap::new();
        m.insert("xs".to_string(), inner);
        m.insert("flag".to_string(), Value::Bool(true));
        let outer = alloc_map(m);
        let j = value_to_json(&outer).expect("normal nesting must serialize");
        assert!(j.is_object());
    }
}
