//! Bitwise builtin implementations.
//!
//! Native integer bit operations backing both the global builtins
//! (`bit_and`/`bit_or`/`bit_xor`/`bit_not`/`bit_shl`/`bit_shr`) and the infix
//! operators `& | ^ << >> ~` (which the parser desugars to these calls).
//!
//! **Semantics:** operands are interpreted as 64-bit signed integers (`i64`).
//! A pblang number is an `f64`, so each operand must be an integral value in
//! the `i64` range; non-integral or out-of-range inputs are a clear error. The
//! result is returned as a number and is exact up to 2^53 (the f64 integer
//! ceiling) — beyond that, ordinary f64 precision applies, same as any large
//! integer literal. For unbounded exact integers use `bigint(...)`. Right shift
//! is arithmetic (sign-preserving), matching Rust's `>>` on `i64`.
//!
//! These are pure computation — no host capabilities — so they are always safe
//! to register; the sandbox does not gate them.
//
// Visibility: every item is `pub(super)` so vm.rs / builtins_table.rs can reach
// them via `fun: bitwise::bit_and` in the BuiltinFn table.

use pblang_ir::{RuntimeError, Value, VmHost};

/// Coerce a pblang number to an `i64` for bit operations, or produce a clear
/// error. Requires a finite, integral value within the `i64` range.
fn to_i64(host: &mut dyn VmHost, v: &Value, fname: &str) -> Result<i64, RuntimeError> {
    match v {
        Value::Number(n) => {
            if !n.is_finite() || n.fract() != 0.0 {
                return Err(host.err(format!(
                    "{fname}(): operand must be an integer, got {n}"
                )));
            }
            if *n < i64::MIN as f64 || *n > i64::MAX as f64 {
                return Err(host.err(format!(
                    "{fname}(): operand {n} is out of the 64-bit integer range"
                )));
            }
            Ok(*n as i64)
        }
        _ => Err(host.err(format!(
            "{fname}(): expected an integer operand"
        ))),
    }
}

/// Coerce a shift amount to `0..=63`. Shifts ≥ 64 (or negative) are rejected so
/// the result is well-defined (Rust panics/UB on out-of-range shift).
fn shift_amount(host: &mut dyn VmHost, v: &Value, fname: &str) -> Result<u32, RuntimeError> {
    let n = to_i64(host, v, fname)?;
    if !(0..=63).contains(&n) {
        return Err(host.err(format!(
            "{fname}(): shift amount must be 0..=63, got {n}"
        )));
    }
    Ok(n as u32)
}

fn arity2(host: &mut dyn VmHost, args: &[Value], fname: &str) -> Result<(), RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "{fname}() expects 2 arguments, got {}",
            args.len()
        )));
    }
    Ok(())
}

/// `bit_and(a, b)` → `a & b` (bitwise AND, backs the `&` operator).
pub(super) fn bit_and(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity2(host, &args, "bit_and")?;
    let a = to_i64(host, &args[0], "bit_and")?;
    let b = to_i64(host, &args[1], "bit_and")?;
    Ok(Value::Number((a & b) as f64))
}

/// `bit_or(a, b)` → `a | b` (bitwise OR, backs the `|` operator).
pub(super) fn bit_or(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity2(host, &args, "bit_or")?;
    let a = to_i64(host, &args[0], "bit_or")?;
    let b = to_i64(host, &args[1], "bit_or")?;
    Ok(Value::Number((a | b) as f64))
}

/// `bit_xor(a, b)` → `a ^ b` (bitwise XOR, backs the `^` operator).
pub(super) fn bit_xor(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity2(host, &args, "bit_xor")?;
    let a = to_i64(host, &args[0], "bit_xor")?;
    let b = to_i64(host, &args[1], "bit_xor")?;
    Ok(Value::Number((a ^ b) as f64))
}

/// `bit_not(a)` → `!a` (bitwise NOT / one's complement, backs the unary `~`).
pub(super) fn bit_not(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("bit_not() expects 1 argument, got {}", args.len())));
    }
    let a = to_i64(host, &args[0], "bit_not")?;
    Ok(Value::Number(!a as f64))
}

/// `bit_shl(a, n)` → `a << n` (left shift, backs the `<<` operator).
pub(super) fn bit_shl(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity2(host, &args, "bit_shl")?;
    let a = to_i64(host, &args[0], "bit_shl")?;
    let n = shift_amount(host, &args[1], "bit_shl")?;
    Ok(Value::Number((a.wrapping_shl(n)) as f64))
}

/// `bit_shr(a, n)` → `a >> n` (arithmetic right shift, backs the `>>` operator).
pub(super) fn bit_shr(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    arity2(host, &args, "bit_shr")?;
    let a = to_i64(host, &args[0], "bit_shr")?;
    let n = shift_amount(host, &args[1], "bit_shr")?;
    Ok(Value::Number((a.wrapping_shr(n)) as f64))
}
