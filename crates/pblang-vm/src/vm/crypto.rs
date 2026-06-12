//! Crypto / constant-time builtin implementations.
//!
//! Free functions backing the crypto builtins registered in
//! `VM::builtins()` (vm.rs):
//!
//!   - `sha256(s)` → hex-encoded SHA-256 digest.
//!   - `hmac_sha256(data, key)` → hex-encoded HMAC-SHA256 (RFC 2104).
//!     Implemented in-house against the `sha2` crate (already a dep) so
//!     we don't add `hmac` to the dependency surface.
//!   - `constant_time_eq(a, b)` → bool. Length-equal byte-string compare
//!     using `acc |= x ^ y` to avoid the early-exit timing leak of `==`.
//!
//! Crypto ops are always available — pure computation, no I/O.

use sha2::Digest;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

pub(super) fn sha256(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!("sha256() expects 1 argument, got {}", args.len())));
    }
    let s = args[0]
        .as_str()
        .map_err(|m| host.err(format!("sha256(): {m}")))?;
    let digest = sha2::Sha256::digest(s.as_bytes());
    Ok(alloc_string(hex::encode(digest)))
}

pub(super) fn constant_time_eq(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "constant_time_eq() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let a = args[0]
        .as_str()
        .map_err(|m| host.err(format!("constant_time_eq(): first arg {m}")))?;
    let b = args[1]
        .as_str()
        .map_err(|m| host.err(format!("constant_time_eq(): second arg {m}")))?;
    let aa = a.as_bytes();
    let bb = b.as_bytes();
    let ok = aa.len() == bb.len()
        && aa
            .iter()
            .zip(bb.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0;
    Ok(Value::Bool(ok))
}

/// hmac_sha256(data, key) — compute HMAC-SHA256 of data with key.
/// Returns a hex-encoded string. Pure computation; not gated.
pub(super) fn hmac_sha256(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "hmac_sha256() expects 2 arguments (data, key), got {}",
            args.len()
        )));
    }
    let data = args[0]
        .as_str()
        .map_err(|m| host.err(format!("hmac_sha256(): {m}")))?;
    let key = args[1]
        .as_str()
        .map_err(|m| host.err(format!("hmac_sha256(): {m}")))?;
    // HMAC-SHA256: K XOR opad || SHA256(K XOR ipad || data)
    // RFC 2104 implementation using only sha2 crate (already a dep).
    use sha2::Sha256;
    const BLOCK_SIZE: usize = 64;
    let key_bytes = key.as_bytes();
    let mut k: Vec<u8> = if key_bytes.len() > BLOCK_SIZE {
        let mut h = Sha256::new();
        h.update(key_bytes);
        h.finalize().to_vec()
    } else {
        key_bytes.to_vec()
    };
    k.resize(BLOCK_SIZE, 0);
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(data.as_bytes());
    let inner_hash = inner.finalize();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(inner_hash);
    let result = outer.finalize();
    Ok(alloc_string(hex::encode(result)))
}
