//! Encoding/decoding builtin implementations.
//!
//! Free functions backing the encoding builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `base64_encode(s)` / `base64_decode(s)` — RFC 4648 standard
//!     alphabet, padding required. `base64_decode_bytes(s)` is the binary
//!     variant returning an array of byte numbers when the decoded payload
//!     is not valid UTF-8.
//!   - `hex_encode(s)` / `hex_decode(s)` — lowercase hex.
//!   - `url_encode(s)` / `url_decode(s)` — percent-encoding per RFC 3986
//!     unreserved set + `+` ↔ ` ` decode for form-encoded payloads.
//!
//! Pure computation — no host capabilities. Always registered.
//
// Visibility: every item is `pub(super)` so vm.rs reaches them via
// `fun: encoding::base64_encode` in the BuiltinFn table.

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_array;

// -------------------------------------------------------------------------
// Base64
// -------------------------------------------------------------------------

pub(super) fn base64_encode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "base64_encode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("base64_encode(): {m}")))?;
    Ok(alloc_string(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        s.as_bytes(),
    )))
}

/// `base64_encode_bytes(arr)` → string: base64 of a raw byte array (0..255).
///
/// Companion to `base64_decode_bytes`. Use this to serialize binary payloads
/// (XOR ciphertext, nonces, hashes) that are not valid UTF-8 and therefore
/// cannot pass through the string-based `base64_encode`.
pub(super) fn base64_encode_bytes(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "base64_encode_bytes() expects 1 argument, got {}",
            args.len()
        )));
    }
    let arr = match &args[0] {
        Value::Array(a) => a.clone(),
        _ => return Err(host.err("base64_encode_bytes(): expected a byte array".into())),
    };
    let mut bytes: Vec<u8> = Vec::with_capacity(arr.borrow().len());
    for v in arr.borrow().iter() {
        match v {
            Value::Number(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= 255.0 => {
                bytes.push(*n as u8);
            }
            _ => {
                return Err(host.err("base64_encode_bytes(): expected byte numbers 0..255".into()))
            }
        }
    }
    Ok(alloc_string(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &bytes,
    )))
}

pub(super) fn base64_decode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "base64_decode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("base64_decode(): {m}")))?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
        .map_err(|e| host.err(format!("base64_decode(): {e}")))?;
    let s = String::from_utf8(bytes)
        .map_err(|e| host.err(format!("base64_decode(): invalid UTF-8: {e}")))?;
    Ok(alloc_string(s))
}

/// base64_decode_bytes(s) → array<number 0-255>
/// BUG FIX #22: base64_decode() fails on binary data; this variant returns raw bytes.
pub(super) fn base64_decode_bytes(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "base64_decode_bytes() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("base64_decode_bytes(): {m}")))?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
        .map_err(|e| host.err(format!("base64_decode_bytes(): {e}")))?;
    Ok(alloc_array(
        bytes.into_iter().map(|b| Value::Number(b as f64)).collect(),
    ))
}

// -------------------------------------------------------------------------
// Hex
// -------------------------------------------------------------------------

pub(super) fn hex_encode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "hex_encode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("hex_encode(): {m}")))?;
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        out.push_str(&format!("{:02x}", b));
    }
    Ok(alloc_string(out))
}

pub(super) fn hex_decode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "hex_decode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("hex_decode(): {m}")))?;
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(host.err("hex_decode(): odd-length hex string".into()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for chunk in bytes.chunks(2) {
        let h = (chunk[0] as char).to_digit(16).ok_or_else(|| {
            host.err(format!(
                "hex_decode(): invalid hex char '{}'",
                chunk[0] as char
            ))
        })?;
        let l = (chunk[1] as char).to_digit(16).ok_or_else(|| {
            host.err(format!(
                "hex_decode(): invalid hex char '{}'",
                chunk[1] as char
            ))
        })?;
        out.push(((h << 4) | l) as u8);
    }
    let s_out = String::from_utf8(out)
        .map_err(|e| host.err(format!("hex_decode(): invalid UTF-8: {e}")))?;
    Ok(alloc_string(s_out))
}

// -------------------------------------------------------------------------
// URL percent-encoding
// -------------------------------------------------------------------------

pub(super) fn url_encode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "url_encode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("url_encode(): {m}")))?;
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let unreserved = matches!(*b,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    Ok(alloc_string(out))
}

pub(super) fn url_decode(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "url_decode() expects 1 argument, got {}",
            args.len()
        )));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("url_decode(): {m}")))?;
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(host.err("url_decode(): truncated escape sequence".into()));
                }
                let h = (bytes[i + 1] as char).to_digit(16).ok_or_else(|| {
                    host.err(format!(
                        "url_decode(): invalid escape char '{}'",
                        bytes[i + 1] as char
                    ))
                })?;
                let l = (bytes[i + 2] as char).to_digit(16).ok_or_else(|| {
                    host.err(format!(
                        "url_decode(): invalid escape char '{}'",
                        bytes[i + 2] as char
                    ))
                })?;
                out.push(((h << 4) | l) as u8);
                i += 3;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    let s_out = String::from_utf8(out)
        .map_err(|e| host.err(format!("url_decode(): invalid UTF-8: {e}")))?;
    Ok(alloc_string(s_out))
}
