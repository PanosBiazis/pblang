# TLS Notes — pblang Web Layer

## Goal
Add HTTPS/TLS support to pblang's HTTP server without embedding a full TLS
stack inside the VM.

## Preferred approach: Host-bridge capability

The cleanest, future-proof design is a **host-provided TLS capability**:
the embedding app (Rust) wraps `TcpStream` in `rustls::StreamOwned` before
passing the handle to the VM. The VM sees the same `tcp_read`/`tcp_write`
builtins — no VM changes required.

```rust
// Host side (Rust)
use rustls::{ServerConfig, StreamOwned};
let tls_stream = StreamOwned::new(server_conn, tcp_stream);
// Wrap in stream trait and register as handle
```

## Alternative: FFI plugin
Load `rustls` or `openssl` as a FFI DLL from pblang script:
```pblang
let tls = ffi.load("pblang_tls.dll");
let s = tls.wrap_server(raw_socket, cert_path, key_path);
```
This requires a thin Rust crate `pblang-tls` that exposes a C ABI.

## Non-goals
- No TLS handshake implemented in pure pblang script
- No OpenSSL bundled in the repo
- No breaking changes to tcp_listen/tcp_accept

## Required VM change (minimal)
- A `pblangStream` trait (Read + Write) so both plain and TLS streams share
  the same handle type in the VM
- `tcp_listen_tls(addr, cert_pem, key_pem)` builtin stub (sandboxed by net cap)

## Status: Implemented (rustls)

As of v0.5, TLS builtins are implemented using `rustls`:

- `tcp_listen_tls(addr, cert_pem_path, key_pem_path)` — creates a TLS server listener
- `tcp_connect_tls(host, port)` — creates a TLS client connection (verifies certificates using `webpki-roots` + native roots)
- `tcp_wrap_tls(handle, cert_path, key_path)` — upgrades an existing plain TCP stream to TLS server mode

The `stdlib/tls.sapp` wrapper provides a high-level API that gracefully falls back to error maps
when builtins are unavailable.

### Production note

TLS verification is enabled in the current runtime. For internet-facing deployments, still prefer
standard operational hardening (certificate rotation, least-privilege network policy, reverse proxy
termination where needed).

---

## Historical: Stage A prototype notes (kept for context)

### API surface defined

The following functions are exported from `stdlib/tls.sapp` and constitute the
stable TLS API surface for Stage A:

| Function | Signature | Purpose |
|---|---|---|
| `listen_tls` | `(addr, cert_path, key_path)` | Create a TLS-wrapped TCP listener |
| `serve_once_tls` | `(addr, cert_path, key_path, handler)` | Accept one TLS connection and dispatch to `handler` |
| `serve_forever_tls` | `(addr, cert_path, key_path, handler)` | Accept connections in a loop, dispatch each to `handler` |
| `wrap_tls` | `(tcp_handle, cert_path, key_path)` | Upgrade an existing plain TCP handle to TLS |

### Original stub behaviour (historical)

During Stage A these functions returned an **error map** of the form:

```pblang/stdlib/tls.sapp#L1-1
map{ error: "TLS not yet implemented (Stage A stub)." }
```

Returning an error map (rather than `nil` or raising a runtime error) lets
callers check the result at runtime without crashing, and makes the stub
distinguishable from a successful response. Tests in
`tests/semantics/tls_stub_test.sapp` assert this contract.

### Path to full implementation (completed)

The Stage A checklist above is now complete in the current runtime:

1. rustls integration is in place.
2. TLS builtins are wired and capability-gated.
3. `stdlib/tls.sapp` delegates to implemented VM builtins.

The remaining work is operational hardening and broader deployment examples, not
core TLS feature bring-up.
