# Bounded Loopback Service

This PBLang-only example migrates an earlier Python socket experiment. The
original opened `0.0.0.0:8080`, blocked indefinitely in `accept`, logged a peer
address and did not close either socket. The migration binds only
`127.0.0.1`, selects an ephemeral port by default and applies explicit accept,
read, write and connect deadlines.

```text
pblang run examples/bounded_loopback_service/main.sapp
pblang run examples/bounded_loopback_service/live_demo.sapp --allow-ffi
pblang run examples/bounded_loopback_service/benchmark.sapp
```

`service.sapp` separates a portable pure protocol from the live transport. A
request is a newline-terminated JSON envelope with a bounded identifier and a
message of at most 512 bytes. Responses acknowledge only the request ID and
byte count; payload and peer identity are not returned to diagnostics or UI.
Malformed, oversized and control-bearing frames fail closed.

The optional live adapter reuses `stdlib:native_tcp.sapp`. On supported Linux
ABIs it creates nonblocking close-on-exec descriptors, performs a real client
and server round trip and verifies descriptor cleanup. Other platforms retain
the protocol and responsive Web/native/Android UI model but report the live
adapter as unavailable. No portable support is inferred from a Linux-only FFI
test.

This is a local connection and lifecycle example, not an Internet service. It
does not provide remote authentication, TLS, multi-client scheduling, service
discovery or authorization for privileged actions. Those require higher-level
protocols and deployment-specific trust policy.

The original source is represented only by SHA-256
`7a69f736e96c0ed7c576e7832e661eeee20961b5172b8244e86ebd93fde79b73`.
