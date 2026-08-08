# Deadline-Bound Native TCP

`stdlib:native_tcp.sapp` provides nonblocking IPv4 text sockets with explicit
connect, accept, read and write deadlines. It is intended for bounded control-plane
operations where the portable `tcp_connect` call could otherwise wait on an
operating-system connect timeout.

```sapp
let TCP = import("stdlib:native_tcp.sapp");
let listener = TCP.listen_ipv4("127.0.0.1", 0, 16);
let socket = TCP.accept_bounded(listener, 250);
// read/write the accepted socket, then close it before the listener
TCP.close(socket);
TCP.close(listener);
```

The implementation is PBLang-only and currently supports little-endian Linux
x86, x86-64 and ARM64 system ABIs. It opens listeners, clients and accepted
sockets with nonblocking and close-on-exec flags, validates connect completion
with `poll` plus `getpeername`, bounds
text frames to 1 MiB, rejects embedded NUL on writes, validates UTF-8 on reads
and closes both the descriptor and FFI handles explicitly. Binary-safe pointer
copying uses the installed OpenSSL `libcrypto`; unsupported systems fail closed.
Ephemeral listeners use `getsockname` to publish the kernel-assigned port.
Accepted sockets own independent FFI handles, so closing one never invalidates
the listener. `accept_bounded` does not create or retain a timeout worker.

Run the semantic test and example with the trusted-code capability profile:

```sh
pblang test semantics --exact native_tcp_test --fail-fast
pblang run examples/native_tcp/main.sapp --allow-all
```

This API does not provide TLS and must not be used as a confidentiality layer.
Applications must authenticate messages and use TLS 1.3 or another verified
secure transport for sensitive payloads.
