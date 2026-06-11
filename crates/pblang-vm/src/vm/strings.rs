//! String builtin implementations.
//!
//! Free functions backing the `str_*` and char-code builtins registered in
//! `VM::builtins()` (vm.rs). Each one validates arity / argument types and
//! returns either a new heap-allocated string (`alloc_string(...)`), an
//! array of strings (`alloc_array(...)`), a number, or a bool.
//!
//! These are pure computation — no host capabilities (no fs / net / exec).
//! They are always safe to register, the sandbox doesn't gate them.
//
// Visibility: every item is `pub(super)` so vm.rs can reach them via
// `fun: strings::str_contains` in the BuiltinFn table.

use pblang_ir::{alloc_string, Obj, RuntimeError, Value, VmHost};

use super::alloc_array;

const MAX_STRING_ALLOC_BYTES: usize = 10 * 1024 * 1024;

// -------------------------------------------------------------------------
// Simple predicates and trim/split
// -------------------------------------------------------------------------

pub(super) fn str_contains(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_contains() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_contains(): {m}")))?;
    let sub = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_contains(): {m}")))?;
    Ok(Value::Bool(s.contains(sub)))
}

pub(super) fn str_starts_with(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_starts_with() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_starts_with(): {m}")))?;
    let pre = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_starts_with(): {m}")))?;
    Ok(Value::Bool(s.starts_with(pre)))
}

pub(super) fn str_trim(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("str_trim() expects 1 argument, got {}", args.len())));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_trim(): {m}")))?;
    Ok(alloc_string(s.trim().to_string()))
}

pub(super) fn str_split(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_split() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_split(): {m}")))?;
    let delim = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_split(): {m}")))?;
    let parts = if delim.is_empty() {
        s.chars()
            .map(|c| alloc_string(c.to_string()))
            .collect::<Vec<_>>()
    } else {
        s.split(delim)
            .map(|p| alloc_string(p.to_string()))
            .collect()
    };
    Ok(alloc_array(parts))
}

// -------------------------------------------------------------------------
// Slice / index / ends-with / case / join
// -------------------------------------------------------------------------

pub(super) fn str_slice(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "str_slice() expects 3 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_slice(): {m}")))?;
    let start = match args[1] {
        Value::Number(n) if n >= 0.0 => n as usize,
        _ => return Err(host.err("str_slice(): start must be a non-negative number".into())),
    };
    let end = match args[2] {
        Value::Number(n) if n >= 0.0 => n as usize,
        _ => return Err(host.err("str_slice(): end must be a non-negative number".into())),
    };
    let chars: Vec<char> = s.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len());
    if start >= end {
        return Ok(alloc_string(""));
    }
    let result: String = chars[start..end].iter().collect();
    Ok(alloc_string(result))
}

/// `slice(seq, start, end)` — polymorphic half-open slice for strings and
/// arrays. Backs the `seq[a..b]` / `seq[a..]` / `seq[..b]` / `seq[..]` syntax;
/// a `nil` bound means 0 (start) or len (end). Strings slice by codepoint
/// (consistent with `len`/`str_slice`/string indexing); arrays slice by
/// element. Out-of-range bounds clamp; `start >= end` yields empty.
pub(super) fn slice(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "slice() expects 3 arguments (seq, start, end), got {}",
            args.len()
        )));
    }
    let bound = |v: &Value, name: &str| -> Result<Option<usize>, RuntimeError> {
        match v {
            Value::Nil => Ok(None),
            Value::Number(n) if n.is_finite() && *n >= 0.0 => Ok(Some(*n as usize)),
            _ => Err(host.err(format!(
                "slice(): {name} must be a non-negative number or nil"
            ))),
        }
    };
    let start_opt = bound(&args[1], "start")?;
    let end_opt = bound(&args[2], "end")?;
    match &args[0] {
        Value::Obj(o) => match o.as_ref() {
            Obj::String(s) => {
                let chars: Vec<char> = s.chars().collect();
                let n = chars.len();
                let start = start_opt.unwrap_or(0).min(n);
                let end = end_opt.unwrap_or(n).min(n);
                if start >= end {
                    return Ok(alloc_string(String::new()));
                }
                Ok(alloc_string(chars[start..end].iter().collect::<String>()))
            }
            _ => Err(host.err("slice(): expected a string or array".into())),
        },
        Value::Array(a) => {
            let a = a.borrow();
            let n = a.len();
            let start = start_opt.unwrap_or(0).min(n);
            let end = end_opt.unwrap_or(n).min(n);
            if start >= end {
                return Ok(alloc_array(Vec::new()));
            }
            Ok(alloc_array(a[start..end].to_vec()))
        }
        _ => Err(host.err("slice(): expected a string or array".into())),
    }
}

pub(super) fn str_index_of(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_index_of() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_index_of(): {m}")))?;
    let sub = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_index_of(): {m}")))?;
    match s.find(sub) {
        Some(byte_idx) => {
            // Convert byte offset to Unicode char index.
            let char_idx = s[..byte_idx].chars().count();
            Ok(Value::Number(char_idx as f64))
        }
        None => Ok(Value::Number(-1.0)),
    }
}

pub(super) fn str_ends_with(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_ends_with() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_ends_with(): {m}")))?;
    let suffix = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_ends_with(): {m}")))?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

pub(super) fn str_lower(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "str_lower() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_lower(): {m}")))?;
    Ok(alloc_string(s.to_lowercase()))
}

pub(super) fn str_upper(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "str_upper() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_upper(): {m}")))?;
    Ok(alloc_string(s.to_uppercase()))
}

pub(super) fn str_join(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "str_join() expects 2 arguments (array, separator), got {}",
            args.len()
        )));
    }
    let arr = match &args[0] {
        Value::Array(a) => a.borrow().clone(),
        _ => return Err(host.err("str_join(): first argument must be an array".into())),
    };
    let sep = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_join(): {m}")))?;
    let parts: Vec<String> = arr
        .iter()
        .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_default())
        .collect();
    Ok(alloc_string(parts.join(sep)))
}

// -------------------------------------------------------------------------
// Char-code helpers
// -------------------------------------------------------------------------

pub(super) fn strfromcharcode(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "strfromcharcode() expects 1 argument, got {}",
            args.len()
        )));
    }
    match &args[0] {
        Value::Number(n) => {
            let cp = *n as u32;
            match char::from_u32(cp) {
                Some(c) => Ok(alloc_string(c.to_string())),
                None => Err(host.err(format!(
                    "strfromcharcode(): {} is not a valid unicode code point",
                    cp
                ))),
            }
        }
        _ => Err(host.err("strfromcharcode(): expected number".into())),
    }
}

/// `strcharcodeat(s, i)` → the Unicode scalar value of the character at
/// **codepoint** index `i`.
///
/// Indexing is by codepoint, consistent with `len`, string indexing `s[i]`,
/// `slice`/`str_slice`, and `str_index_of` — the entire pblang string API
/// treats a string as a sequence of codepoints, not bytes. The returned value
/// is the full Unicode scalar (not a byte), so
/// `strfromcharcode(strcharcodeat(s, i))` round-trips for every character,
/// including non-ASCII. For pure-ASCII strings this is identical to the old
/// byte-indexed behaviour (codepoint index == byte index, scalar == byte). For
/// raw byte access on binary data, use `str_to_bytes(s)` and index the array.
pub(super) fn strcharcodeat(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "strcharcodeat() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("strcharcodeat(): {m}")))?;
    let idx = match &args[1] {
        Value::Number(n) => *n as usize,
        _ => return Err(host.err("strcharcodeat(): index must be a number".into())),
    };
    match s.chars().nth(idx) {
        Some(c) => Ok(Value::Number(c as u32 as f64)),
        None => Err(host.err(format!(
            "strcharcodeat(): index {} out of bounds (len {})",
            idx,
            s.chars().count()
        ))),
    }
}

/// `str_to_bytes(s)` → array<number 0..255>: the raw UTF-8 bytes of `s`.
///
/// pblang strings are guaranteed UTF-8, so binary-aware code must work on
/// byte arrays rather than codepoints. This is the inverse of `bytes_to_str`.
pub(super) fn str_to_bytes(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "str_to_bytes() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_to_bytes(): {m}")))?;
    Ok(alloc_array(
        s.as_bytes()
            .iter()
            .map(|b| Value::Number(*b as f64))
            .collect(),
    ))
}

/// `bytes_to_str(arr)` → string: decode an array of bytes (0..255) as UTF-8.
///
/// Errors if the bytes are not valid UTF-8. Inverse of `str_to_bytes`.
pub(super) fn bytes_to_str(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "bytes_to_str() expects 1 argument, got {}",
            args.len()
        )));
    }
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("bytes_to_str(): expected a byte array".into())),
    };
    let mut bytes: Vec<u8> = Vec::with_capacity(arr.borrow().len());
    for v in arr.borrow().iter() {
        match v {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 => {
                bytes.push(*n as u8);
            }
            _ => return Err(host.err("bytes_to_str(): expected byte numbers 0..255".into())),
        }
    }
    let s = String::from_utf8(bytes)
        .map_err(|e| host.err(format!("bytes_to_str(): invalid UTF-8: {e}")))?;
    Ok(alloc_string(s))
}

// -------------------------------------------------------------------------
// Replace / format / repeat / pad
// -------------------------------------------------------------------------

/// `str_replace_all(s, from, to)` — replace all occurrences of `from` with
/// `to` in `s`. Faster and safer than `regex_replace_all` for literals.
pub(super) fn str_replace_all(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err(format!(
            "str_replace_all() expects 3 arguments (string, from, to), got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
    let from = args[1]
        .as_str()
        .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
    let to = args[2]
        .as_str()
        .map_err(|m| host.err(format!("str_replace_all(): {m}")))?;
    Ok(alloc_string(s.replace(from, to)))
}

/// `str_format(fmt, ...args)` — Python-style %s / %d / %f / %v / %% formatting.
/// Example: `str_format("Hello %s, you are %d years old!", name, age)`.
pub(super) fn str_format(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(host.err("str_format() expects at least 1 argument (format string)".into()));
    }
    let fmt = args[0]
        .as_str()
        .map_err(|_| host.err("str_format(): first argument must be a string".into()))?;

    let mut result = String::with_capacity(fmt.len() * 2);
    let mut arg_idx = 1usize;
    let mut chars = fmt.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            if let Some(&spec) = chars.peek() {
                chars.next();
                match spec {
                    's' => {
                        if arg_idx >= args.len() {
                            return Err(host.err("str_format(): not enough arguments".into()));
                        }
                        result.push_str(&format!("{}", args[arg_idx]));
                        arg_idx += 1;
                    }
                    'd' | 'i' => {
                        if arg_idx >= args.len() {
                            return Err(host.err("str_format(): not enough arguments".into()));
                        }
                        match &args[arg_idx] {
                            Value::Number(n) => result.push_str(&format!("{}", *n as i64)),
                            v => result.push_str(&format!("{v}")),
                        }
                        arg_idx += 1;
                    }
                    'f' => {
                        if arg_idx >= args.len() {
                            return Err(host.err("str_format(): not enough arguments".into()));
                        }
                        match &args[arg_idx] {
                            Value::Number(n) => result.push_str(&format!("{:.6}", n)),
                            v => result.push_str(&format!("{v}")),
                        }
                        arg_idx += 1;
                    }
                    'v' => {
                        // %v = any value (Go-style)
                        if arg_idx >= args.len() {
                            return Err(host.err("str_format(): not enough arguments".into()));
                        }
                        result.push_str(&format!("{}", args[arg_idx]));
                        arg_idx += 1;
                    }
                    '%' => result.push('%'),
                    other => {
                        result.push('%');
                        result.push(other);
                    }
                }
            } else {
                result.push('%');
            }
        } else {
            result.push(ch);
        }
    }
    Ok(alloc_string(result))
}

/// `str_repeat(s, n)` → string repeated `n` times.
pub(super) fn str_repeat(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err("str_repeat(s, n) expects 2 arguments".into()));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("str_repeat(): first arg must be string".into()))?;
    let n = match &args[1] {
        Value::Number(x) if *x >= 0.0 => *x as usize,
        _ => return Err(host.err("str_repeat(): second arg must be non-negative number".into())),
    };
    let bytes = s.len().checked_mul(n).ok_or_else(|| {
        host.err("str_repeat(): repeated string size overflows addressable size".into())
    })?;
    if bytes > MAX_STRING_ALLOC_BYTES {
        return Err(host.err(format!(
            "str_repeat(): result is {bytes} bytes; maximum is {MAX_STRING_ALLOC_BYTES}"
        )));
    }
    Ok(alloc_string(s.repeat(n)))
}

/// `str_pad_left(s, width, pad_char)` → left-pads `s` to `width` with `pad_char`.
pub(super) fn str_pad_left(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err("str_pad_left(s, width, pad) expects 3 arguments".into()));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("str_pad_left(): first arg must be string".into()))?;
    let w = match &args[1] {
        Value::Number(x) => *x as usize,
        _ => return Err(host.err("str_pad_left(): width must be number".into())),
    };
    let pad = args[2]
        .as_str()
        .map_err(|_| host.err("str_pad_left(): pad must be string".into()))?;
    let pad_ch = pad.chars().next().unwrap_or(' ');
    let cur = s.chars().count();
    if cur >= w {
        return Ok(alloc_string(s));
    }
    let padding: String = std::iter::repeat_n(pad_ch, w - cur).collect();
    Ok(alloc_string(format!("{padding}{s}")))
}

/// `str_pad_right(s, width, pad_char)` → right-pads `s` to `width` with `pad_char`.
pub(super) fn str_pad_right(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(host.err("str_pad_right(s, width, pad) expects 3 arguments".into()));
    }
    let s = args[0]
        .as_str()
        .map_err(|_| host.err("str_pad_right(): first arg must be string".into()))?;
    let w = match &args[1] {
        Value::Number(x) => *x as usize,
        _ => return Err(host.err("str_pad_right(): width must be number".into())),
    };
    let pad = args[2]
        .as_str()
        .map_err(|_| host.err("str_pad_right(): pad must be string".into()))?;
    let pad_ch = pad.chars().next().unwrap_or(' ');
    let cur = s.chars().count();
    if cur >= w {
        return Ok(alloc_string(s));
    }
    let padding: String = std::iter::repeat_n(pad_ch, w - cur).collect();
    Ok(alloc_string(format!("{s}{padding}")))
}
