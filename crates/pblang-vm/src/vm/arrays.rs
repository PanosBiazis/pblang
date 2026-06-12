//! Array builtin implementations.
//!
//! Free functions backing the array-flavoured builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `array_push` / `array_pop` — in-place mutation, return new length /
//!     popped value.
//!   - `array_sort` — lexicographic for numbers and strings; other types
//!     compare equal.
//!   - `array_unique` / `array_zip` / `array_chunk` / `array_flatten` /
//!     `array_take` / `array_drop` — non-mutating transforms.
//!   - `range(start, end[, step])` — half-open numeric range.
//!   - `enumerate(arr)` — `[index, value]` pairs.
//!
//! Pure computation — no host capabilities. Always registered.
//
// Visibility: every item is `pub(super)` so vm.rs reaches them via
// `fun: arrays::array_push` in the BuiltinFn table.

use pblang_ir::{RuntimeError, Value, VmHost};

use super::alloc_array;

// -------------------------------------------------------------------------
// Mutating ops
// -------------------------------------------------------------------------

pub(super) fn array_push(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "array_push() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let a = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("array_push(): expected array".into())),
    };
    a.borrow_mut().push(args[1].clone());
    let n = { a.borrow().len() };
    Ok(Value::Number(n as f64))
}

pub(super) fn array_pop(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "array_pop() expects 1 argument, got {}",
            args.len()
        )));
    }
    let a = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("array_pop(): expected array".into())),
    };
    let v = a.borrow_mut().pop().unwrap_or(Value::Nil);
    Ok(v)
}

// -------------------------------------------------------------------------
// Sort / range / enumerate
// -------------------------------------------------------------------------

pub(super) fn array_sort(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "array_sort() expects 1 argument, got {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Array(a) => {
            let mut items = a.borrow().clone();
            items.sort_by(|x, y| match (x, y) {
                (Value::Number(a), Value::Number(b)) => {
                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                }
                (Value::Obj(a), Value::Obj(b)) => {
                    let sa = match a.as_ref() {
                        pblang_ir::Obj::String(s) => s.clone(),
                        pblang_ir::Obj::BigInt(n) => n.to_string(),
                        pblang_ir::Obj::Rational(r) => r.to_string(),
                        pblang_ir::Obj::Decimal(d) => d.to_string(),
                    };
                    let sb = match b.as_ref() {
                        pblang_ir::Obj::String(s) => s.clone(),
                        pblang_ir::Obj::BigInt(n) => n.to_string(),
                        pblang_ir::Obj::Rational(r) => r.to_string(),
                        pblang_ir::Obj::Decimal(d) => d.to_string(),
                    };
                    sa.cmp(&sb)
                }
                _ => std::cmp::Ordering::Equal,
            });
            Ok(alloc_array(items))
        }
        _ => Err(host.err("array_sort(): expected array".into())),
    }
}

/// range(end) → [0,...,end-1]
/// range(start, end) → [start,...,end-1]
/// range(start, end, step) → array with step
pub(super) fn range(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let (start, end, step) = match args.len() {
        1 => {
            let n = match &args[0] {
                Value::Number(x) => *x,
                _ => return Err(host.err("range(): argument must be number".into())),
            };
            (0.0, n, 1.0)
        }
        2 => {
            let s = match &args[0] {
                Value::Number(x) => *x,
                _ => return Err(host.err("range(): arguments must be numbers".into())),
            };
            let e = match &args[1] {
                Value::Number(x) => *x,
                _ => return Err(host.err("range(): arguments must be numbers".into())),
            };
            (s, e, 1.0)
        }
        3 => {
            let s = match &args[0] {
                Value::Number(x) => *x,
                _ => return Err(host.err("range(): arguments must be numbers".into())),
            };
            let e = match &args[1] {
                Value::Number(x) => *x,
                _ => return Err(host.err("range(): arguments must be numbers".into())),
            };
            let st = match &args[2] {
                Value::Number(x) if *x != 0.0 => *x,
                _ => return Err(host.err("range(): step must be non-zero number".into())),
            };
            (s, e, st)
        }
        _ => return Err(host.err(format!("range() expects 1-3 arguments, got {}", args.len()))),
    };
    let mut result = Vec::new();
    let mut i = start;
    if step > 0.0 {
        while i < end {
            result.push(Value::Number(i));
            i += step;
        }
    } else {
        while i > end {
            result.push(Value::Number(i));
            i += step;
        }
    }
    Ok(alloc_array(result))
}

/// enumerate(arr) → array of [index, value] pairs
pub(super) fn enumerate(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "enumerate() expects 1 argument, got {}",
            args.len()
        )));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("enumerate(): argument must be array".into())),
    };
    let result: Vec<Value> = items
        .into_iter()
        .enumerate()
        .map(|(i, v)| alloc_array(vec![Value::Number(i as f64), v]))
        .collect();
    Ok(alloc_array(result))
}

// -------------------------------------------------------------------------
// Non-mutating transforms
// -------------------------------------------------------------------------

/// array_unique(arr) → new array with duplicate values removed (preserves order, first occurrence)
pub(super) fn array_unique(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err("array_unique(arr) expects 1 argument".into()));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("array_unique(): argument must be an array".into())),
    };
    let mut seen: Vec<Value> = vec![];
    let mut result: Vec<Value> = vec![];
    for item in items {
        if !seen.iter().any(|s| s == &item) {
            seen.push(item.clone());
            result.push(item);
        }
    }
    Ok(alloc_array(result))
}

/// array_zip(a, b) → array of [a_i, b_i] pairs (stops at shorter length)
pub(super) fn array_zip(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("array_zip(a, b) expects 2 arguments".into()));
    }
    let a = match &args[0] {
        Value::Array(x) => x.borrow().clone(),
        _ => return Err(host.err("array_zip(): first arg must be array".into())),
    };
    let b = match &args[1] {
        Value::Array(x) => x.borrow().clone(),
        _ => return Err(host.err("array_zip(): second arg must be array".into())),
    };
    let n = a.len().min(b.len());
    let result: Vec<Value> = (0..n)
        .map(|i| alloc_array(vec![a[i].clone(), b[i].clone()]))
        .collect();
    Ok(alloc_array(result))
}

/// array_chunk(arr, size) → array of arrays of at most `size` elements
pub(super) fn array_chunk(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("array_chunk(arr, size) expects 2 arguments".into()));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("array_chunk(): first arg must be array".into())),
    };
    let size = match &args[1] {
        Value::Number(n) if *n >= 1.0 => *n as usize,
        _ => return Err(host.err("array_chunk(): size must be positive number".into())),
    };
    let result: Vec<Value> = items
        .chunks(size)
        .map(|chunk| alloc_array(chunk.to_vec()))
        .collect();
    Ok(alloc_array(result))
}

/// array_flatten(arr) → flattens one level of nesting
pub(super) fn array_flatten(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err("array_flatten(arr) expects 1 argument".into()));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("array_flatten(): argument must be array".into())),
    };
    let mut result: Vec<Value> = vec![];
    for item in items {
        match item {
            Value::Array(sub) => result.extend(sub.borrow().clone()),
            other => result.push(other),
        }
    }
    Ok(alloc_array(result))
}

/// array_take(arr, n) → first n elements
pub(super) fn array_take(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("array_take(arr, n) expects 2 arguments".into()));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("array_take(): first arg must be array".into())),
    };
    let n = match &args[1] {
        Value::Number(x) if *x >= 0.0 => *x as usize,
        _ => return Err(host.err("array_take(): n must be non-negative".into())),
    };
    Ok(alloc_array(items.into_iter().take(n).collect()))
}

/// array_drop(arr, n) → all elements after the first n
pub(super) fn array_drop(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("array_drop(arr, n) expects 2 arguments".into()));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("array_drop(): first arg must be array".into())),
    };
    let n = match &args[1] {
        Value::Number(x) if *x >= 0.0 => *x as usize,
        _ => return Err(host.err("array_drop(): n must be non-negative".into())),
    };
    Ok(alloc_array(items.into_iter().skip(n).collect()))
}
