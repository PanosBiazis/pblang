//! TCP + TLS builtins.
//!
//! Ten free functions backing `tcp_listen` / `tcp_accept` / `tcp_read` /
//! `tcp_read_until` / `tcp_write` / `tcp_close` / `tcp_local_addr` plus the
//! TLS variants `tcp_listen_tls` / `tcp_connect_tls` / `tcp_wrap_tls`.
//!
//! Each builtin:
//!
//! 1. Checks the `net` capability gate (`StdlibConfig.net`) and errors with
//!    `net access denied` if denied.
//! 2. Validates arity.
//! 3. Borrows the underlying `NetResource` out of the global handle table,
//!    does the syscall / TLS handshake, and re-inserts it — so two pblang
//!    threads can't hold the same TcpStream concurrently.
//!
//! TLS uses rustls with webpki-roots + best-effort native roots (the old
//! "accept any cert" verifier was removed in v0.5.1's security audit).
//
// Each function is registered by name in `VM::builtins()` via
// `BuiltinFn { name: "tcp_listen", arity: Some(1), fun: net::tcp_listen }`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};

use pblang_ir::{alloc_string, RuntimeError, Value, VmHost};

use super::errors::vm_cap;
use super::handles::{value_to_handle, with_net_table, NET_NEXT};
use super::{NetResource, NetStream, StreamKind};

/// Plain-text substring search. Used by `tcp_read_until` to find the
/// caller-supplied delimiter inside the read buffer. Returns the offset of
/// the *first* match.
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

pub(super) fn tcp_listen(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_listen", "net"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "tcp_listen() expects 1 argument, got {}",
            args.len()
        )));
    }
    let addr = args[0]
        .as_str()
        .map_err(|m| host.err(format!("tcp_listen(): {m}")))?;
    let l = TcpListener::bind(addr).map_err(|e| host.err(format!("tcp_listen(): {e}")))?;
    let id = with_net_table(host, |t| {
        let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(id, NetResource::Listener(l));
        id
    })?;
    Ok(Value::Number(id as f64))
}

pub(super) fn tcp_accept(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_accept", "net"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "tcp_accept() expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_accept")?;
    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_accept(): invalid handle".into()))?;

    match r {
        NetResource::Listener(listener) => {
            let (s, _addr) = listener
                .accept()
                .map_err(|e| host.err(format!("tcp_accept(): {e}")))?;
            if let Some(ms) = host.io_timeout_ms() {
                let d = Duration::from_millis(ms);
                let _ = s.set_read_timeout(Some(d));
                let _ = s.set_write_timeout(Some(d));
            }
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Listener(listener)));
            let sid = with_net_table(host, |t| {
                let sid = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(
                    sid,
                    NetResource::Stream(Box::new(NetStream {
                        inner: StreamKind::Plain(s),
                        buf: vec![],
                    })),
                );
                sid
            })?;
            Ok(Value::Number(sid as f64))
        }
        NetResource::TlsListener(listener, tls_config) => {
            // TLS accept: accept TCP, then perform TLS handshake
            let (tcp_stream, _addr) = listener
                .accept()
                .map_err(|e| host.err(format!("tcp_accept(): {e}")))?;
            if let Some(ms) = host.io_timeout_ms() {
                let d = Duration::from_millis(ms);
                let _ = tcp_stream.set_read_timeout(Some(d));
                let _ = tcp_stream.set_write_timeout(Some(d));
            }
            let server_conn = ServerConnection::new(tls_config.clone())
                .map_err(|e| host.err(format!("tcp_accept() TLS: {e}")))?;
            let tls_stream = StreamOwned::new(server_conn, tcp_stream);

            let _ = with_net_table(host, |t| {
                t.insert(id, NetResource::TlsListener(listener, tls_config))
            });
            let sid = with_net_table(host, |t| {
                let sid = NET_NEXT.fetch_add(1, Ordering::Relaxed);
                t.insert(
                    sid,
                    NetResource::Stream(Box::new(NetStream {
                        inner: StreamKind::TlsServer(tls_stream),
                        buf: vec![],
                    })),
                );
                sid
            })?;
            Ok(Value::Number(sid as f64))
        }
        other => {
            let _ = with_net_table(host, |t| t.insert(id, other));
            Err(host.err("tcp_accept(): handle is not a listener".into()))
        }
    }
}

pub(super) fn tcp_read(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_read", "net"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "tcp_read() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_read")?;
    let n = match &args[1] {
        Value::Number(x) if x.is_finite() && *x >= 0.0 => *x as usize,
        _ => return Err(host.err("tcp_read(): expected byte count number".into())),
    };

    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_read(): invalid handle".into()))?;
    let mut st = match r {
        NetResource::Stream(s) => *s,
        other => {
            let _ = with_net_table(host, |t| t.insert(id, other));
            return Err(host.err("tcp_read(): handle is not a stream".into()));
        }
    };

    // serve from buffer first
    let mut out = Vec::<u8>::new();
    if !st.buf.is_empty() {
        let take = st.buf.len().min(n);
        out.extend_from_slice(&st.buf[..take]);
        st.buf.drain(..take);
    }

    while out.len() < n {
        let mut tmp = vec![0u8; (n - out.len()).min(4096)];
        let read = st
            .inner
            .read(&mut tmp)
            .map_err(|e| host.err(format!("tcp_read(): {e}")))?;
        if read == 0 {
            break;
        }
        out.extend_from_slice(&tmp[..read]);
    }

    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(Box::new(st))));
    Ok(alloc_string(String::from_utf8_lossy(&out).to_string()))
}

pub(super) fn tcp_read_until(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_read_until", "net"));
    }
    if args.len() != 3 {
        return Err(host.err(format!(
            "tcp_read_until() expects 3 arguments, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_read_until")?;
    let delim = args[1]
        .as_str()
        .map_err(|m| host.err(format!("tcp_read_until(): {m}")))?
        .as_bytes()
        .to_vec();
    let max = match &args[2] {
        Value::Number(x) if x.is_finite() && *x >= 0.0 => *x as usize,
        _ => return Err(host.err("tcp_read_until(): expected max_bytes number".into())),
    };

    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_read_until(): invalid handle".into()))?;
    let mut st = match r {
        NetResource::Stream(s) => *s,
        other => {
            let _ = with_net_table(host, |t| t.insert(id, other));
            return Err(host.err("tcp_read_until(): handle is not a stream".into()));
        }
    };

    let mut total = 0usize;
    loop {
        if let Some(pos) = find_subslice(&st.buf, &delim) {
            let end = (pos + delim.len()).min(st.buf.len());
            let out = st.buf.drain(..end).collect::<Vec<u8>>();
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(Box::new(st))));
            return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
        }
        if total >= max {
            let out = st.buf.drain(..).collect::<Vec<u8>>();
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(Box::new(st))));
            return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
        }
        let mut tmp = [0u8; 4096];
        let read = st
            .inner
            .read(&mut tmp)
            .map_err(|e| host.err(format!("tcp_read_until(): {e}")))?;
        if read == 0 {
            let out = st.buf.drain(..).collect::<Vec<u8>>();
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(Box::new(st))));
            return Ok(alloc_string(String::from_utf8_lossy(&out).to_string()));
        }
        st.buf.extend_from_slice(&tmp[..read]);
        total += read;
    }
}

pub(super) fn tcp_write(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_write", "net"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "tcp_write() expects 2 arguments, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_write")?;
    let s = args[1]
        .as_str()
        .map_err(|m| host.err(format!("tcp_write(): {m}")))?;

    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_write(): invalid handle".into()))?;
    let mut st = match r {
        NetResource::Stream(s) => *s,
        other => {
            let _ = with_net_table(host, |t| t.insert(id, other));
            return Err(host.err("tcp_write(): handle is not a stream".into()));
        }
    };

    let n = st
        .inner
        .write(s.as_bytes())
        .map_err(|e| host.err(format!("tcp_write(): {e}")))?;
    let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(Box::new(st))));
    Ok(Value::Number(n as f64))
}

pub(super) fn tcp_close(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_close", "net"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "tcp_close() expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_close")?;
    let _ = with_net_table(host, |t| t.remove(&id));
    Ok(Value::Nil)
}

pub(super) fn tcp_local_addr(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_local_addr", "net"));
    }
    if args.len() != 1 {
        return Err(host.err(format!(
            "tcp_local_addr() expects 1 argument, got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_local_addr")?;
    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_local_addr(): invalid handle".into()))?;
    match r {
        NetResource::Listener(l) => {
            let addr = l
                .local_addr()
                .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Listener(l)));
            Ok(alloc_string(addr.to_string()))
        }
        NetResource::TlsListener(l, cfg) => {
            let addr = l
                .local_addr()
                .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
            let _ = with_net_table(host, |t| t.insert(id, NetResource::TlsListener(l, cfg)));
            Ok(alloc_string(addr.to_string()))
        }
        NetResource::Stream(s) => {
            let addr = s
                .inner
                .local_addr()
                .map_err(|e| host.err(format!("tcp_local_addr(): {e}")))?;
            let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(s)));
            Ok(alloc_string(addr.to_string()))
        }
    }
}

// ---- TLS builtins (W4-gamma) ----

/// `tcp_listen_tls(addr, cert_pem_path, key_pem_path)` → listener handle.
/// Creates a TLS-wrapped TCP listener using rustls.
pub(super) fn tcp_listen_tls(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_listen_tls", "net"));
    }
    if args.len() != 3 {
        return Err(host.err(format!(
            "tcp_listen_tls() expects 3 arguments (addr, cert_path, key_path), got {}",
            args.len()
        )));
    }
    let addr = args[0]
        .as_str()
        .map_err(|m| host.err(format!("tcp_listen_tls(): {m}")))?;
    let cert_path = args[1]
        .as_str()
        .map_err(|m| host.err(format!("tcp_listen_tls(): cert_path: {m}")))?;
    let key_path = args[2]
        .as_str()
        .map_err(|m| host.err(format!("tcp_listen_tls(): key_path: {m}")))?;

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| host.err(format!("tcp_listen_tls(): open cert: {e}")))?;
    let mut cert_reader = std::io::BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .filter_map(|r| r.ok())
        .collect();
    if certs.is_empty() {
        return Err(host.err("tcp_listen_tls(): no certificates found in PEM file".into()));
    }

    let key_file = std::fs::File::open(key_path)
        .map_err(|e| host.err(format!("tcp_listen_tls(): open key: {e}")))?;
    let mut key_reader = std::io::BufReader::new(key_file);
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| host.err(format!("tcp_listen_tls(): read key: {e}")))?
        .ok_or_else(|| host.err("tcp_listen_tls(): no private key found in PEM file".into()))?;

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| host.err(format!("tcp_listen_tls(): config: {e}")))?;

    let listener =
        TcpListener::bind(addr).map_err(|e| host.err(format!("tcp_listen_tls(): bind: {e}")))?;

    let id = with_net_table(host, |t| {
        let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(
            id,
            NetResource::TlsListener(listener, Arc::new(server_config)),
        );
        id
    })?;

    Ok(Value::Number(id as f64))
}

/// `tcp_connect_tls(host, port)` → stream handle.
/// Connects to a TLS server with certificate verification enabled. Trust
/// roots come from webpki-roots plus best-effort native/system roots.
pub(super) fn tcp_connect_tls(
    host: &mut dyn VmHost,
    args: Vec<Value>,
) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_connect_tls", "net"));
    }
    if args.len() != 2 {
        return Err(host.err(format!(
            "tcp_connect_tls() expects 2 arguments (host, port), got {}",
            args.len()
        )));
    }
    let host_str = args[0]
        .as_str()
        .map_err(|m| host.err(format!("tcp_connect_tls(): host: {m}")))?;
    let port = match &args[1] {
        Value::Number(n) if n.is_finite() && *n >= 0.0 && *n <= 65535.0 => *n as u16,
        _ => return Err(host.err("tcp_connect_tls(): port must be a number 0-65535".into())),
    };

    let addr = format!("{}:{}", host_str, port);
    let tcp_stream = TcpStream::connect(&addr)
        .map_err(|e| host.err(format!("tcp_connect_tls(): connect: {e}")))?;
    if let Some(ms) = host.io_timeout_ms() {
        let d = Duration::from_millis(ms);
        let _ = tcp_stream.set_read_timeout(Some(d));
        let _ = tcp_stream.set_write_timeout(Some(d));
    }

    // Build ClientConfig with proper system root certs (webpki-roots +
    // native). This is what replaced the old DangerousAllowAllVerifier
    // that accepted ANY certificate (v0.5.1 security audit).
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let native_certs = rustls_native_certs::load_native_certs();
    for cert in native_certs.certs {
        let _ = root_store.add(cert);
    }
    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let server_name: ServerName<'static> = host_str
        .to_string()
        .try_into()
        .map_err(|_| host.err("tcp_connect_tls(): invalid server name for SNI".into()))?;

    let client_conn = ClientConnection::new(Arc::new(client_config), server_name)
        .map_err(|e| host.err(format!("tcp_connect_tls(): TLS: {e}")))?;
    let tls_stream = StreamOwned::new(client_conn, tcp_stream);

    let id = with_net_table(host, |t| {
        let id = NET_NEXT.fetch_add(1, Ordering::Relaxed);
        t.insert(
            id,
            NetResource::Stream(Box::new(NetStream {
                inner: StreamKind::TlsClient(tls_stream),
                buf: vec![],
            })),
        );
        id
    })?;

    Ok(Value::Number(id as f64))
}

/// `tcp_wrap_tls(handle, cert_path, key_path)` → new stream handle.
/// Upgrades an existing plain TCP stream to TLS server mode (re-uses the
/// same handle id).
pub(super) fn tcp_wrap_tls(host: &mut dyn VmHost, args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !host.allow_net() {
        return Err(vm_cap!(host, "tcp_wrap_tls", "net"));
    }
    if args.len() != 3 {
        return Err(host.err(format!(
            "tcp_wrap_tls() expects 3 arguments (handle, cert_path, key_path), got {}",
            args.len()
        )));
    }
    let id = value_to_handle(host, args[0].clone(), "tcp_wrap_tls")?;
    let cert_path = args[1]
        .as_str()
        .map_err(|m| host.err(format!("tcp_wrap_tls(): cert_path: {m}")))?;
    let key_path = args[2]
        .as_str()
        .map_err(|m| host.err(format!("tcp_wrap_tls(): key_path: {m}")))?;

    let r = with_net_table(host, |t| t.remove(&id))?
        .ok_or_else(|| host.err("tcp_wrap_tls(): invalid handle".into()))?;

    let tcp_stream = match r {
        NetResource::Stream(st) => match st.inner {
            StreamKind::Plain(s) => s,
            _ => {
                let _ = with_net_table(host, |t| t.insert(id, NetResource::Stream(st)));
                return Err(
                    host.err("tcp_wrap_tls(): handle must be a plain (non-TLS) TCP stream".into())
                );
            }
        },
        other => {
            let _ = with_net_table(host, |t| t.insert(id, other));
            return Err(
                host.err("tcp_wrap_tls(): handle must be a plain (non-TLS) TCP stream".into())
            );
        }
    };

    let cert_file = std::fs::File::open(cert_path)
        .map_err(|e| host.err(format!("tcp_wrap_tls(): open cert: {e}")))?;
    let mut cert_reader = std::io::BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .filter_map(|r| r.ok())
        .collect();
    if certs.is_empty() {
        return Err(host.err("tcp_wrap_tls(): no certificates found in PEM file".into()));
    }

    let key_file = std::fs::File::open(key_path)
        .map_err(|e| host.err(format!("tcp_wrap_tls(): open key: {e}")))?;
    let mut key_reader = std::io::BufReader::new(key_file);
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| host.err(format!("tcp_wrap_tls(): read key: {e}")))?
        .ok_or_else(|| host.err("tcp_wrap_tls(): no private key found in PEM file".into()))?;

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| host.err(format!("tcp_wrap_tls(): config: {e}")))?;

    let server_conn = ServerConnection::new(Arc::new(server_config))
        .map_err(|e| host.err(format!("tcp_wrap_tls(): TLS: {e}")))?;
    let tls_stream = StreamOwned::new(server_conn, tcp_stream);

    let _ = with_net_table(host, |t| {
        t.insert(
            id,
            NetResource::Stream(Box::new(NetStream {
                inner: StreamKind::TlsServer(tls_stream),
                buf: vec![],
            })),
        )
    });

    Ok(Value::Number(id as f64))
}
