//! Regex builtin implementations.
//!
//! Free functions backing the `regex_*` builtins registered in
//! `VM::builtins()` (vm.rs). Each one compiles its pattern with the
//! `regex` crate (RE2-style; no backreferences or look-around) and
//! returns the appropriate match/replace shape.
//!
//!   - `regex_match(s, pat)` → bool.
//!   - `regex_find(s, pat)` → string | nil (first match).
//!   - `regex_find_all(s, pat)` → array of strings (non-overlapping).
//!   - `regex_replace(s, pat, rep)` / `regex_replace_all(...)` → string.
//!   - `regex_split(s, pat)` → array of strings.
//!   - `regex_capture(s, pat)` → array of group strings (group 0 is the
//!     whole match) | nil. Optional captures show as `Value::Nil`.
//!
//! Pure computation — no host capabilities. Always registered.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_array;

// ── Regex builtins ─────────────────────────────────────────────────────

/// regex_match(str, pattern) → bool
/// Returns true if str contains a match for the regex pattern.
pub(super) fn regex_match_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "regex_match() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_match(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_match(): second arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_match(): invalid pattern: {e}")))?;
    Ok(Value::Bool(re.is_match(s)))
}

/// regex_find(str, pattern) → string | nil
/// Returns the first match, or nil if no match.
pub(super) fn regex_find_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "regex_find() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_find(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_find(): second arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_find(): invalid pattern: {e}")))?;
    match re.find(s) {
        Some(m) => Ok(alloc_string(m.as_str())),
        None => Ok(Value::Nil),
    }
}

/// regex_find_all(str, pattern) → array of strings
/// Returns all non-overlapping matches.
pub(super) fn regex_find_all_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "regex_find_all() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_find_all(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_find_all(): second arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_find_all(): invalid pattern: {e}")))?;
    let matches: Vec<Value> = re.find_iter(s).map(|m| alloc_string(m.as_str())).collect();
    Ok(alloc_array(matches))
}

/// regex_replace(str, pattern, replacement) → string
/// Replaces the first match with replacement.
pub(super) fn regex_replace_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "regex_replace() expects 3 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_replace(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_replace(): second arg must be string".into()))?;
    let rep = args[2]
        .as_str()
        .map_err(|_| host.err("regex_replace(): third arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_replace(): invalid pattern: {e}")))?;
    Ok(alloc_string(re.replacen(s, 1, rep).to_string()))
}

/// regex_replace_all(str, pattern, replacement) → string
/// Replaces all matches with replacement.
pub(super) fn regex_replace_all_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "regex_replace_all() expects 3 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_replace_all(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_replace_all(): second arg must be string".into()))?;
    let rep = args[2]
        .as_str()
        .map_err(|_| host.err("regex_replace_all(): third arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_replace_all(): invalid pattern: {e}")))?;
    Ok(alloc_string(re.replace_all(s, rep).to_string()))
}

/// regex_split(str, pattern) → array of strings
/// Splits str on every match of pattern.
pub(super) fn regex_split_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "regex_split() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_split(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_split(): second arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_split(): invalid pattern: {e}")))?;
    let parts: Vec<Value> = re.split(s).map(alloc_string).collect();
    Ok(alloc_array(parts))
}

/// regex_capture(str, pattern) → array of captured group strings | nil
/// Returns an array of capture groups for the first match, or nil if no match.
/// Group 0 is the entire match; groups 1+ are parenthesized captures.
pub(super) fn regex_capture_builtin(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "regex_capture() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("regex_capture(): first arg must be string".into()))?;
    let pat = args[1]
        .as_str()
        .map_err(|_| host.err("regex_capture(): second arg must be string".into()))?;
    let re = regex::Regex::new(pat)
        .map_err(|e| host.err(format!("regex_capture(): invalid pattern: {e}")))?;
    match re.captures(s) {
        None => Ok(Value::Nil),
        Some(caps) => {
            let groups: Vec<Value> = caps
                .iter()
                .map(|m| match m {
                    Some(m) => alloc_string(m.as_str()),
                    None => Value::Nil,
                })
                .collect();
            Ok(alloc_array(groups))
        }
    }
}
