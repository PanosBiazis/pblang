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
- `tcp_listen_tls(addr, cert_pem, key_pem)` builtin entrypoint (sandboxed by net cap)

## Status: Builtins implemented; high-level API currently fail-closed

As of v0.5, TLS builtins are implemented using `rustls`:

- `tcp_listen_tls(addr, cert_pem_path, key_pem_path)` — creates a TLS server listener
- `tcp_connect_tls(host, port)` — creates a TLS client connection (verifies certificates using `webpki-roots` + native roots)
- `tcp_wrap_tls(handle, cert_path, key_path)` — upgrades an existing plain TCP stream to TLS server mode

The `stdlib/tls.sapp` wrapper provides a high-level API that returns error maps
when builtins are unavailable or the process has not selected exactly one
Rustls `CryptoProvider`.

### Production note

Certificate verification exists in the client implementation, but a live
2026-07-19 server test found that the current dependency graph enables both
`ring` and `aws-lc-rs`. Calling `ServerConfig::builder()` before a process-wide
provider is selected causes a Rustls panic. PBLang `try/catch` cannot catch a
host panic, so `stdlib:tls.sapp` now defaults to unavailable and returns a clear
error instead of invoking the builtin.

An embedding host that has installed exactly one provider may set
`PBLANG_TLS_PROVIDER_READY=ring` or `aws-lc-rs` before running PBLang code. This
environment flag is a host contract, not provider installation by itself;
setting it incorrectly can still expose the underlying runtime defect. Until
the host startup owns provider selection globally, prefer verified external TLS
termination or Tor for remote services.

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

### Original fallback behaviour (historical)

During Stage A these functions returned an **error map** of the form:

```pblang/stdlib/tls.sapp#L1-1
map{ error: "TLS unavailable in Stage A fallback mode." }
```

Returning an error map (rather than `nil` or raising a runtime error) lets
callers check the result at runtime without crashing, and makes the fallback
distinguishable from a successful response. Tests in
the TLS legacy-fallback semantic test asserts this contract.

### Path to safe default availability

1. Keep the existing rustls builtins and capability gates.
2. Select exactly one provider once during host startup, before any TLS call.
3. Add live listener/client tests with CA verification and malformed-handshake cases.
4. Remove the temporary environment contract only after startup selection is universal.
