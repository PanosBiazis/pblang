//! HTTP / WebSocket builtin implementations.
//!
//! Free functions backing the `http_*` and `websocket_*`/`ws_*` builtins
//! registered in `VM::builtins()` (vm.rs).
//!
//!   - `http_get(url)` / `http_post(url, body[, content_type])` — outbound
//!     `ureq` calls; honour `host.io_timeout_ms()`.
//!   - `http_get_auth(url, token)` — outbound GET with `Bearer` header.
//!   - `http_post_auth(url, headers, body)` — outbound POST with caller-
//!     supplied headers (map<string,string>).
//!   - `http_parse_request(raw)` — split a raw `request-line + headers +
//!     "\r\n\r\n" + body` text into `{method, path, headers, body}`.
//!   - `http_response(status, body)` / `http_response_ct(status, ct, body)`
//!     / `http_response_json(status, value)` — serialize a response with
//!     `Content-Length` + `Connection: close`.
//!   - `websocket_accept_key(key)` — RFC 6455 magic-string + SHA-1 +
//!     base64.
//!   - `ws_parse_frame(frame)` / `ws_write_frame(opcode, payload)` —
//!     RFC 6455 frame codec (chars-as-bytes to round-trip through pblang
//!     strings).
//!
//! Outbound HTTP gates on `host.allow_http()`; server-side parse/response
//! helpers gate on `host.allow_net()`; the RFC-6455 helpers are pure.

use std::collections::HashMap;
use std::time::Duration;

use base64;
use rand::Rng;
use serde_json as sj;
use sha1::Digest as _;

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::alloc_map;
use super::errors::vm_cap;
use super::json::value_to_json;

// -------------------------------------------------------------------------
// Outbound (ureq)
// -------------------------------------------------------------------------

pub(super) fn http_get(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_http() {
        return Err(vm_cap!(host, "http_get", "http"));
    }
    if args.len() != 1 {
        return Err(host.err(format!("http_get() expects 1 argument, got {}", args.len())));
    }
    let url = args[0]
        .as_str()
        .map_err(|m| host.err(format!("http_get(): {m}")))?;
    let req = ureq::get(url);
    let req = if let Some(ms) = host.io_timeout_ms() {
        req.timeout(Duration::from_millis(ms))
    } else {
        req
    };
    let resp = req
        .call()
        .map_err(|e| host.err(format!("http_get(): {e}")))?;
    let body = resp
        .into_string()
        .map_err(|e| host.err(format!("http_get(): {e}")))?;
    Ok(alloc_string(body))
}

/// http_post(url, body) or http_post(url, body, content_type)
/// BUG FIX #23: No longer hardcodes Content-Type: application/json.
/// If 3rd argument (content_type) is omitted, defaults to application/json
/// for backward compatibility. Pass e.g. "text/plain" for non-JSON bodies.
pub(super) fn http_post(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_http() {
        return Err(vm_cap!(host, "http_post", "http"));
    }
    if args.len() < 2 || args.len() > 3 {
        return Err(host.err(format!(
            "http_post() expects 2 or 3 arguments (url, body[, content_type]), got {}",
            args.len()
        )));
    }
    let url = args[0]
        .as_str()
        .map_err(|m| host.err(format!("http_post(): {m}")))?;
    let body = args[1]
        .as_str()
        .map_err(|m| host.err(format!("http_post(): {m}")))?;
    let ct = if args.len() == 3 {
        args[2]
            .as_str()
            .map_err(|m| host.err(format!("http_post(): {m}")))?
            .to_string()
    } else {
        "application/json".to_string()
    };
    let req = ureq::post(url).set("content-type", &ct);
    let req = if let Some(ms) = host.io_timeout_ms() {
        req.timeout(Duration::from_millis(ms))
    } else {
        req
    };
    let resp = req
        .send_string(body)
        .map_err(|e| host.err(format!("http_post(): {e}")))?;
    let out = resp
        .into_string()
        .map_err(|e| host.err(format!("http_post(): {e}")))?;
    Ok(alloc_string(out))
}

pub(super) fn http_get_auth(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 2 {
        return Err(host.err(format!(
            "http_get_auth() expects 2 arguments (url, token), got {}",
            args.len()
        )));
    }
    if !host.allow_http() {
        return Err(vm_cap!(host, "http_get_auth", "http"));
    }
    let url = args[0]
        .as_str()
        .map_err(|m| host.err(format!("http_get_auth(): {m}")))?;
    let token = args[1]
        .as_str()
        .map_err(|m| host.err(format!("http_get_auth(): token: {m}")))?;
    let mut req = ureq::get(url).set("Authorization", &format!("Bearer {token}"));
    if let Some(ms) = host.io_timeout_ms() {
        req = req.timeout(Duration::from_millis(ms));
    }
    match req.call() {
        Ok(resp) => Ok(alloc_string(resp.into_string().unwrap_or_default())),
        Err(e) => Ok(alloc_string(format!("error: {e}"))),
    }
}

pub(super) fn http_post_auth(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() < 3 {
        return Err(host.err(format!(
            "http_post_auth() expects 3 arguments, got {}",
            args.len()
        )));
    }
    if !host.allow_http() {
        return Err(vm_cap!(host, "http_post_auth", "http"));
    }
    let url = args[0]
        .as_str()
        .map_err(|m| host.err(format!("http_post_auth(): {m}")))?;
    let body_str = args[2]
        .as_str()
        .map_err(|m| host.err(format!("http_post_auth(): body: {m}")))?;
    // Collect headers first to avoid borrow conflicts.
    let headers: Vec<(String, String)> = if let Value::Map(ref hm) = args[1] {
        hm.borrow()
            .iter()
            .filter_map(|(k, v)| v.as_str().ok().map(|s| (k.clone(), s.to_string())))
            .collect()
    } else {
        vec![]
    };
    let mut req = ureq::post(url);
    for (k, v) in &headers {
        req = req.set(k.as_str(), v.as_str());
    }
    let req = if let Some(ms) = host.io_timeout_ms() {
        req.timeout(Duration::from_millis(ms))
    } else {
        req
    };
    match req.send_string(body_str) {
        Ok(resp) => Ok(alloc_string(resp.into_string().unwrap_or_default())),
        Err(e) => Ok(alloc_string(format!("error: {e}"))),
    }
}

// -------------------------------------------------------------------------
// Server-side: parse request / build response
// -------------------------------------------------------------------------

fn http_status_reason(status: i64) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "Unknown",
    }
}

pub(super) fn http_parse_request(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "http_parse_request", "net"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "http_parse_request() expects 1 argument, got {}",
            args.len()
        )));
    }
    let raw = args[0]
        .as_str()
        .map_err(|m| host.err(format!("http_parse_request(): {m}")))?;
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.split("\r\n");
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::<String, Value>::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_string(), alloc_string(v.trim().to_string()));
        }
    }
    let headers_v = alloc_map(headers);

    let mut out = HashMap::<String, Value>::new();
    out.insert("method".into(), alloc_string(method));
    out.insert("path".into(), alloc_string(path));
    out.insert("headers".into(), headers_v);
    out.insert("body".into(), alloc_string(body.to_string()));
    Ok(alloc_map(out))
}

pub(super) fn http_response(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "http_response() expects 2 arguments (status, body), got {}",
            args.len()
        )));
    }
    let status = match &args[0] {
        Value::Number(n) if n.is_finite() => *n as i64,
        _ => return Err(host.err("http_response(): expected status number".into())),
    };
    let body = args[1]
        .as_str()
        .map_err(|m| host.err(format!("http_response(): {m}")))?;
    let reason = http_status_reason(status);
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    Ok(alloc_string(resp))
}

pub(super) fn http_response_ct(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "http_response_ct", "net"));
    }
    if args.len() != 3 {
        return Err(host.err(format!(
            "http_response_ct() expects 3 arguments (status, content_type, body), got {}",
            args.len()
        )));
    }
    let status = match &args[0] {
        Value::Number(n) if n.is_finite() => *n as i64,
        _ => return Err(host.err("http_response_ct(): expected status number".into())),
    };
    let ct = args[1]
        .as_str()
        .map_err(|m| host.err(format!("http_response_ct(): {m}")))?;
    let body = args[2]
        .as_str()
        .map_err(|m| host.err(format!("http_response_ct(): {m}")))?;
    let reason = http_status_reason(status);
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    Ok(alloc_string(resp))
}

pub(super) fn http_response_json(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "http_response_json", "net"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "http_response_json() expects 2 arguments (status, value), got {}",
            args.len()
        )));
    }
    let status = match &args[0] {
        Value::Number(n) if n.is_finite() => *n as i64,
        _ => return Err(host.err("http_response_json(): expected status number".into())),
    };
    let j = value_to_json(&args[1]).map_err(|m| host.err(m))?;
    let body = sj::to_string(&j).map_err(|e| host.err(format!("http_response_json(): {e}")))?;
    let ct = "application/json";
    let reason = http_status_reason(status);
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    Ok(alloc_string(resp))
}

// -------------------------------------------------------------------------
// WebSocket (RFC 6455)
// -------------------------------------------------------------------------

pub(super) fn websocket_accept_key(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "websocket_accept_key() expects 1 argument, got {}",
            args.len()
        )));
    }
    let key = args[0]
        .as_str()
        .map_err(|m| host.err(format!("websocket_accept_key(): {m}")))?;
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined = format!("{key}{WS_GUID}");
    let hash = sha1::Sha1::digest(combined.as_bytes());
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash.as_slice());
    Ok(alloc_string(encoded))
}

pub(super) fn ws_parse_frame(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(host.err(format!(
            "ws_parse_frame() expects 1 argument, got {}",
            args.len()
        )));
    }
    let frame = args[0]
        .as_str()
        .map_err(|m| host.err(format!("ws_parse_frame(): {m}")))?;
    let codes: Vec<u32> = frame.chars().map(|c| c as u32).collect();
    if codes.len() < 2 {
        return Err(host.err("ws_parse_frame(): frame too short".into()));
    }

    let b0 = codes[0] as u8;
    let b1 = codes[1] as u8;
    let fin = if (b0 & 0x80) != 0 { 1.0 } else { 0.0 };
    let opcode = (b0 & 0x0f) as f64;
    let masked = (b1 & 0x80) != 0;
    let len_code = (b1 & 0x7f) as usize;
    let mut idx = 2usize;
    let payload_len = if len_code <= 125 {
        len_code
    } else if len_code == 126 {
        if codes.len() < idx + 2 {
            return Err(host.err("ws_parse_frame(): frame too short for 16-bit length".into()));
        }
        let n = ((codes[idx] as usize) << 8) | (codes[idx + 1] as usize);
        idx += 2;
        n
    } else {
        if codes.len() < idx + 8 {
            return Err(host.err("ws_parse_frame(): frame too short for 64-bit length".into()));
        }
        let mut n: u64 = 0;
        for i in 0..8 {
            n = (n << 8) | (codes[idx + i] as u64);
        }
        idx += 8;
        usize::try_from(n)
            .map_err(|_| host.err("ws_parse_frame(): payload too large for this runtime".into()))?
    };

    let mut masking_key = [0u8; 4];
    if masked {
        if codes.len() < idx + 4 {
            return Err(host.err("ws_parse_frame(): masked frame missing key".into()));
        }
        for i in 0..4 {
            masking_key[i] = codes[idx + i] as u8;
        }
        idx += 4;
    }
    if codes.len() < idx + payload_len {
        return Err(host.err("ws_parse_frame(): truncated payload".into()));
    }

    let mut payload = String::with_capacity(payload_len);
    for i in 0..payload_len {
        let mut b = codes[idx + i] as u8;
        if masked {
            b ^= masking_key[i % 4];
        }
        let ch = std::char::from_u32(b as u32)
            .ok_or_else(|| host.err("ws_parse_frame(): invalid payload byte".into()))?;
        payload.push(ch);
    }

    let mut m = HashMap::new();
    m.insert("opcode".into(), Value::Number(opcode));
    m.insert("payload".into(), alloc_string(payload));
    m.insert("fin".into(), Value::Number(fin));
    m.insert(
        "masked".into(),
        Value::Number(if masked { 1.0 } else { 0.0 }),
    );
    m.insert("payload_len".into(), Value::Number(payload_len as f64));
    m.insert("frame_len".into(), Value::Number((idx + payload_len) as f64));
    m.insert("next_index".into(), Value::Number((idx + payload_len) as f64));
    Ok(alloc_map(m))
}

pub(super) fn ws_write_frame(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(host.err(format!(
            "ws_write_frame() expects 2 arguments (opcode, payload), got {}",
            args.len()
        )));
    }
    let opcode = match args[0] {
        Value::Number(n) if n.is_finite() && (0.0..=15.0).contains(&n) => n as u8,
        _ => return Err(host.err("ws_write_frame(): opcode must be number 0-15".into())),
    };
    let payload = args[1]
        .as_str()
        .map_err(|m| host.err(format!("ws_write_frame(): payload: {m}")))?;
    let payload_bytes: Vec<u8> = payload.chars().map(|c| (c as u32).min(255) as u8).collect();

    let mut rng = rand::thread_rng();
    let mask_key = [rng.gen::<u8>(), rng.gen::<u8>(), rng.gen::<u8>(), rng.gen::<u8>()];
    let mut out = String::new();
    let b0 = 0x80u8 | (opcode & 0x0f); // FIN=1
    let plen = payload_bytes.len();
    let mut len_ext: Vec<u8> = Vec::new();
    let b1 = if plen <= 125 {
        0x80u8 | (plen as u8) // MASK=1 + short length
    } else if plen <= 65_535 {
        len_ext.push(((plen >> 8) & 0xff) as u8);
        len_ext.push((plen & 0xff) as u8);
        0x80u8 | 126u8
    } else {
        let plen64 = plen as u64;
        for shift in (0..=56).rev().step_by(8) {
            len_ext.push(((plen64 >> shift) & 0xff) as u8);
        }
        0x80u8 | 127u8
    };
    out.push(
        std::char::from_u32(b0 as u32)
            .ok_or_else(|| host.err("ws_write_frame(): invalid header byte".into()))?,
    );
    out.push(
        std::char::from_u32(b1 as u32)
            .ok_or_else(|| host.err("ws_write_frame(): invalid header byte".into()))?,
    );
    for b in len_ext {
        out.push(
            std::char::from_u32(b as u32)
                .ok_or_else(|| host.err("ws_write_frame(): invalid extended length byte".into()))?,
        );
    }
    for b in mask_key {
        out.push(
            std::char::from_u32(b as u32)
                .ok_or_else(|| host.err("ws_write_frame(): invalid mask key byte".into()))?,
        );
    }
    for (i, b) in payload_bytes.iter().enumerate() {
        let masked_b = *b ^ mask_key[i % 4];
        out.push(
            std::char::from_u32(masked_b as u32)
                .ok_or_else(|| host.err("ws_write_frame(): invalid masked payload byte".into()))?,
        );
    }
    Ok(alloc_string(out))
}

