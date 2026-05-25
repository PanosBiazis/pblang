//! Parsing/conversion builtin implementations.
//!
//! Free functions backing the parse_* builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `parse_number(s)` — `s.trim().parse::<f64>()`.
//!   - `parse_bool(s)` — case-insensitive `"true"` / `"false"`.
//!   - `parse_sse_line(s)` — split a Server-Sent Events line into
//!     `{field, value}`. Recognises `data:`, `event:`, `id:`, `retry:`
//!     and returns `field=""` for anything else.
//!
//! Pure computation — no host capabilities. Always registered.

use std::collections::HashMap;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_map;

pub(super) fn parse_number(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "parse_number() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("parse_number(): {m}")))?;
    let n: f64 = s
        .trim()
        .parse()
        .map_err(|e| host.err(format!("parse_number(): {e}")))?;
    Ok(Value::Number(n))
}

pub(super) fn parse_bool(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "parse_bool() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("parse_bool(): {m}")))?;
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        _ => Err(host.err("parse_bool(): expected 'true' or 'false'".into())),
    }
}

pub(super) fn parse_sse_line(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "parse_sse_line() expects 1 argument, got {}",
            args.len()
        )));
    }
    let line = args[0]
        .as_str()
        .map_err(|m| host.err(format!("parse_sse_line(): {m}")))?;
    let (field, value) = if let Some(v) = line.strip_prefix("data: ") {
        ("data", v)
    } else if let Some(v) = line.strip_prefix("event: ") {
        ("event", v)
    } else if let Some(v) = line.strip_prefix("id: ") {
        ("id", v)
    } else if let Some(v) = line.strip_prefix("retry: ") {
        ("retry", v)
    } else {
        ("", line)
    };
    let mut m = HashMap::new();
    m.insert("field".into(), alloc_string(field));
    m.insert("value".into(), alloc_string(value));
    Ok(alloc_map(m))
}
