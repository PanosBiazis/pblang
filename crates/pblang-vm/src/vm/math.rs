//! Math builtin implementations.
//!
//! Free functions backing the math-flavoured builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `num_sqrt` / `num_pow` / `num_ln` / `num_exp` / `num_atan2` /
//!     `num_mod` / `num_floor` / `num_ceil` / `num_abs` / `num_max` /
//!     `num_min` — scalar f64 ops.
//!   - `checked_add` / `checked_sub` / `checked_mul` / `checked_div` —
//!     overflow/zero-checked arithmetic, returning `{ok, value|error}` maps.
//!   - `sum` / `min_of` / `max_of` / `product` — array reductions.
//!   - `clamp(v, min, max)`.
//!   - `random_bytes(n)` / `random_float()` — non-crypto RNG via `rand 0.8`
//!     thread-local source.
//!
//! These are pure computation — no host capabilities (no fs / net / exec).
//! They are always safe to register; the sandbox doesn't gate them.
//
// Visibility: every item is `pub(super)` so vm.rs can reach them via
// `fun: math::num_sqrt` in the BuiltinFn table.

use std::collections::HashMap;

use rand::Rng;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::{alloc_array, alloc_map};

// -------------------------------------------------------------------------
// Scalar f64 ops
// -------------------------------------------------------------------------

pub(super) fn num_sqrt(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("num_sqrt() expects 1 argument, got {}", args.len())));
    }
    let n = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_sqrt(): expected number".into())),
    };
    Ok(Value::Number(n.sqrt()))
}

pub(super) fn num_pow(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("num_pow() expects 2 arguments, got {}", args.len())));
    }
    let a = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_pow(): expected number base".into())),
    };
    let b = match args[1] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_pow(): expected number exponent".into())),
    };
    Ok(Value::Number(a.powf(b)))
}

pub(super) fn num_ln(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("num_ln() expects 1 argument, got {}", args.len())));
    }
    let n = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_ln(): expected number".into())),
    };
    Ok(Value::Number(n.ln()))
}

pub(super) fn num_exp(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("num_exp() expects 1 argument, got {}", args.len())));
    }
    let n = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_exp(): expected number".into())),
    };
    Ok(Value::Number(n.exp()))
}

pub(super) fn num_atan2(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "num_atan2() expects 2 arguments (y,x), got {}",
            args.len()
        )));
    }
    let y = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_atan2(): expected number y".into())),
    };
    let x = match args[1] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_atan2(): expected number x".into())),
    };
    Ok(Value::Number(y.atan2(x)))
}

pub(super) fn num_mod(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("num_mod() expects 2 arguments, got {}", args.len())));
    }
    let a = match args[0] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_mod(): expected number a".into())),
    };
    let b = match args[1] {
        Value::Number(n) => n,
        _ => return Err(host.err("num_mod(): expected number b".into())),
    };
    Ok(Value::Number(a % b))
}

pub(super) fn num_floor(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "num_floor() expects 1 argument, got {}",
            args.len()
        )));
    }
    let n = match args[0] {
        Value::Number(x) => x,
        _ => return Err(host.err("num_floor(): expected number".into())),
    };
    Ok(Value::Number(n.floor()))
}

pub(super) fn num_ceil(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("num_ceil() expects 1 argument, got {}", args.len())));
    }
    let n = match args[0] {
        Value::Number(x) => x,
        _ => return Err(host.err("num_ceil(): expected number".into())),
    };
    Ok(Value::Number(n.ceil()))
}

pub(super) fn num_abs(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("num_abs() expects 1 argument, got {}", args.len())));
    }
    match &args[0] {
        Value::Number(n) => Ok(Value::Number(n.abs())),
        _ => Err(host.err("num_abs(): expected number".into())),
    }
}

pub(super) fn num_max(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("num_max() expects 2 arguments, got {}", args.len())));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(*b))),
        _ => Err(host.err("num_max(): expected numbers".into())),
    }
}

pub(super) fn num_min(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!("num_min() expects 2 arguments, got {}", args.len())));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(*b))),
        _ => Err(host.err("num_min(): expected numbers".into())),
    }
}

// -------------------------------------------------------------------------
// Overflow / zero-checked arithmetic
// -------------------------------------------------------------------------

/// checked_add(a, b) — add two numbers; returns Err result map if overflow.
pub(super) fn checked_add(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("checked_add(a, b) takes 2 arguments".into()));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => {
            let result = a + b;
            if result.is_infinite() && a.is_finite() && b.is_finite() {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(false));
                    m.insert(
                        "error".to_string(),
                        alloc_string("arithmetic overflow in checked_add"),
                    );
                    m
                }))
            } else {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(true));
                    m.insert("value".to_string(), Value::Number(result));
                    m
                }))
            }
        }
        _ => Err(host.err("checked_add: both arguments must be numbers".into())),
    }
}

pub(super) fn checked_sub(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("checked_sub(a, b) takes 2 arguments".into()));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => Ok(alloc_map({
            let mut m = HashMap::new();
            m.insert("ok".to_string(), Value::Bool(true));
            m.insert("value".to_string(), Value::Number(a - b));
            m
        })),
        _ => Err(host.err("checked_sub: both arguments must be numbers".into())),
    }
}

pub(super) fn checked_mul(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("checked_mul(a, b) takes 2 arguments".into()));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => {
            let result = a * b;
            if result.is_infinite() && a.is_finite() && b.is_finite() {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(false));
                    m.insert(
                        "error".to_string(),
                        alloc_string("arithmetic overflow in checked_mul"),
                    );
                    m
                }))
            } else {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(true));
                    m.insert("value".to_string(), Value::Number(result));
                    m
                }))
            }
        }
        _ => Err(host.err("checked_mul: both arguments must be numbers".into())),
    }
}

pub(super) fn checked_div(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("checked_div(a, b) takes 2 arguments".into()));
    }
    match (&args[0], &args[1]) {
        (Value::Number(a), Value::Number(b)) => {
            if *b == 0.0 {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(false));
                    m.insert(
                        "error".to_string(),
                        alloc_string("division by zero in checked_div"),
                    );
                    m
                }))
            } else {
                Ok(alloc_map({
                    let mut m = HashMap::new();
                    m.insert("ok".to_string(), Value::Bool(true));
                    m.insert("value".to_string(), Value::Number(a / b));
                    m
                }))
            }
        }
        _ => Err(host.err("checked_div: both arguments must be numbers".into())),
    }
}

// -------------------------------------------------------------------------
// Array reductions
// -------------------------------------------------------------------------

/// sum(arr) → numeric sum of array (nil/non-numeric elements are skipped)
pub(super) fn sum(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("sum() expects 1 argument, got {}", args.len())));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("sum(): argument must be array".into())),
    };
    let s: f64 = items
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .sum();
    Ok(Value::Number(s))
}

/// min_of(arr) → minimum numeric value, or nil if empty
pub(super) fn min_of(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("min_of() expects 1 argument, got {}", args.len())));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("min_of(): argument must be array".into())),
    };
    let nums: Vec<f64> = items
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();
    if nums.is_empty() {
        return Ok(Value::Nil);
    }
    Ok(Value::Number(
        nums.iter().cloned().fold(f64::INFINITY, f64::min),
    ))
}

/// max_of(arr) → maximum numeric value, or nil if empty
pub(super) fn max_of(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("max_of() expects 1 argument, got {}", args.len())));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("max_of(): argument must be array".into())),
    };
    let nums: Vec<f64> = items
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect();
    if nums.is_empty() {
        return Ok(Value::Nil);
    }
    Ok(Value::Number(
        nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ))
}

/// product(arr) → product of all numeric elements
pub(super) fn product(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("product() expects 1 argument, got {}", args.len())));
    }
    let items = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("product(): argument must be array".into())),
    };
    let p: f64 = items
        .iter()
        .filter_map(|v| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .product();
    Ok(Value::Number(p))
}

/// clamp(val, min, max) → value clamped to [min, max]
pub(super) fn clamp(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err("clamp(val, min, max) expects 3 arguments".into()));
    }
    let v = match &args[0] {
        Value::Number(x) => *x,
        _ => return Err(host.err("clamp(): arguments must be numbers".into())),
    };
    let mn = match &args[1] {
        Value::Number(x) => *x,
        _ => return Err(host.err("clamp(): arguments must be numbers".into())),
    };
    let mx = match &args[2] {
        Value::Number(x) => *x,
        _ => return Err(host.err("clamp(): arguments must be numbers".into())),
    };
    Ok(Value::Number(v.max(mn).min(mx)))
}

// -------------------------------------------------------------------------
// RNG (non-crypto)
// -------------------------------------------------------------------------

pub(super) fn random_bytes(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "random_bytes() expects 1 argument, got {}",
            args.len()
        )));
    }
    let n = match args[0] {
        Value::Number(x) if x >= 0.0 && x <= (usize::MAX as f64) => x as usize,
        _ => return Err(host.err("random_bytes(): expected non-negative integer".into())),
    };
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
    Ok(alloc_array(
        bytes.into_iter().map(|b| Value::Number(b as f64)).collect(),
    ))
}

pub(super) fn random_float(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !args.is_empty() {
        return Err(host.err(format!(
            "random_float() expects 0 arguments, got {}",
            args.len()
        )));
    }
    let mut rng = rand::thread_rng();
    Ok(Value::Number(rng.gen::<f64>()))
}
