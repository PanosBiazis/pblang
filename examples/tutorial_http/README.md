# Tutorial — HTTP server

A "hello world" HTTP server returning JSON.

## Run

```bash
pblang run examples/tutorial_http/main.sapp
```

Then in another shell:

```bash
curl http://127.0.0.1:8081
# → {"message":"hello from pblang"}
```

Stop with `Ctrl-C`.

## What you learn

- `stdlib:http_server.sapp` — request routing built on top of the VM's
  `tcp_listen` + `tcp_accept` builtins.
- Handler shape: a function `fn(req) -> map{ status, content_type, body }`.
- `serve_forever(addr, handler)` blocks the main thread accepting connections.

## Going further

- Add routing: `if req.path == "/users" { ... } else { ... }`.
- Read the JSON request body via `req.body`.
- For HTTPS, swap `tcp_listen` for `tcp_listen_tls` (see `stdlib:tls.sapp` and
  `docs/TLS_notes.md`).
- For middleware (logging, auth, CORS), see `stdlib:middleware.sapp` and
  `stdlib:web_framework.sapp`.
- For high-throughput, run multiple worker VMs sharing a TCP listener handle
  via `stdlib:thread.sapp`.

## Sandboxing

If you embed pblang in a host that runs untrusted scripts, set
`StdlibConfig { net: false, http: false, .. }` to block all networking —
`http_server.serve_forever` will then fail at the `tcp_listen` call instead
of binding a port the embedder didn't authorise.
