# TLS Notes — Sapphire Web Layer

## Goal
Add HTTPS/TLS support to Sapphire's HTTP server without embedding a full TLS
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
// Wrap in SapphireStream trait and register as handle
```

## Alternative: FFI plugin
Load `rustls` or `openssl` as a FFI DLL from Sapphire script:
```sapphire
let tls = ffi.load("sapphire_tls.dll");
let s = tls.wrap_server(raw_socket, cert_path, key_path);
```
This requires a thin Rust crate `sapphire-tls` that exposes a C ABI.

## Non-goals
- No TLS handshake implemented in pure Sapphire
- No OpenSSL bundled in the repo
- No breaking changes to tcp_listen/tcp_accept

## Required VM change (minimal)
- A `SapphireStream` trait (Read + Write) so both plain and TLS streams share
  the same handle type in the VM
- `tcp_listen_tls(addr, cert_pem, key_pem)` builtin stub (sandboxed by net cap)

## Status: Research / Design — not implemented

---

## Stage A — Prototype (W4-gamma)

### API surface defined

The following functions are exported from `stdlib/tls.sapp` and constitute the
stable TLS API surface for Stage A:

| Function | Signature | Purpose |
|---|---|---|
| `listen_tls` | `(addr, cert_path, key_path)` | Create a TLS-wrapped TCP listener |
| `serve_once_tls` | `(addr, cert_path, key_path, handler)` | Accept one TLS connection and dispatch to `handler` |
| `serve_forever_tls` | `(addr, cert_path, key_path, handler)` | Accept connections in a loop, dispatch each to `handler` |
| `wrap_tls` | `(tcp_handle, cert_path, key_path)` | Upgrade an existing plain TCP handle to TLS |

### Stub behaviour

All four functions currently return an **error map** of the form:

```pblang/stdlib/tls.sapp#L1-1
map{ error: "TLS not yet implemented (Stage A stub)." }
```

Returning an error map (rather than `nil` or raising a runtime error) lets
callers check the result at runtime without crashing, and makes the stub
distinguishable from a successful response. Tests in
`tests/semantics/tls_stub_test.sapp` assert this contract.

### Path to full implementation

1. Add `rustls` dependency to the host Rust crate (`Cargo.toml`).
2. Implement `SapphireStream` trait wrapping `rustls::StreamOwned`.
3. Wire `listen_tls` and `wrap_tls` as VM builtins gated by the `net`
   capability, replacing the stub error maps with real handle objects.
4. `serve_once_tls` / `serve_forever_tls` can remain in pure Sapphire script
   once `listen_tls` and `wrap_tls` are real builtins.
5. Remove the Stage A error-map returns; update this document to Stage B.
