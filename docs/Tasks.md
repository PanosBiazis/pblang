# pblang Active Backlog

Active tasks only. History belongs in `docs/Report.md`.

**Goal**: pblang evolves as a production platform — embedded + standalone,
AI/Quantum/Web/Polyglot-ready, future-proof, no artificial limits, test-driven
steps, stable core.

**Current priority order:**
TLS/HTTPS → E5 Executables → Match Guard Fix → LSP → Cranelift JIT → Registry → RE → Spec → Editor → Framework/OS

**Current status**: 83/84 semantics tests passing (1 known parser limitation: match guard separator)

---

## ✅ Recently Completed (v0.5)

- **Security**: MIT License filled in, `cargo-audit` added to CI, all Clippy warnings fixed
- **Error handling**: `CompileError` type, `compile()` returns `Result<Chunk, CompileError>` everywhere
- **Safety**: `unsafe` FFI functions properly marked, SAFETY comments added
- **Code quality**: Zero warnings, `Default` impl for VM, type aliases for complex types
- **New stdlib modules**: `result.sapp`, `option.sapp`, `iter.sapp`, `event.sapp`, `cache.sapp`, `validate.sapp`, `uuid.sapp`
- **Improved stdlib**: `strings.sapp` (+reverse, camelCase, snakeCase, format, lstrip, rstrip, index_of, is_numeric), `time.sapp` (+stopwatch, duration constants, format_duration_ms), `http.sapp` (+retry, build_query_string, get_auth, post_json_auth), `os.sapp` (+platform, home_dir, temp_dir, which, getenv, require_env)
- **tls.sapp**: Upgraded from pure stub to real dispatch to `tcp_listen_tls`/`tcp_connect_tls` builtins
- **rustls + rustls-pemfile**: Added to `pblang-vm/Cargo.toml` — TLS builtin implementation ready

---

## Pending Tasks

### W4 — TLS (highest priority production blocker)

**W4-gamma full** — Implement `tcp_listen_tls` / `tcp_connect_tls` VM builtins using rustls
- Dependencies added: `rustls = "0.23"`, `rustls-pemfile = "2.1"` ✅
- Implement `SapphireStream` trait (wraps `TcpStream` or `rustls::StreamOwned<ServerConnection, TcpStream>`)
- Add `tcp_listen_tls(addr, cert_pem_path, key_pem_path)` builtin
- Add `tcp_connect_tls(host, port)` builtin for HTTPS client
- Add `tcp_wrap_tls(handle, cert_path, key_path)` for upgrading existing handles
- Update `docs/TLS_notes.md` → Stage B
- Test: `tls_server_hello_pass` (self-signed cert smoke test)

---

### E5 — Executables

**E5-gamma** — Self-extracting binary (Stage A)
- `crates/pblang-stub/src/main.rs`: self-extraction logic (sentinel + tarball)
- `pblang make-self-extracting [path] [--out <binary>]` subcommand
- Test: `self_extract_binary_contains_sentinel`
- Docs: `docs/Native_backend_notes.md` Stage A → "implemented"

---

### SP1 — Match Guard Fix

**SP1-gamma** — Separator disambiguation
- Root cause: chumsky 0.8's `parse_recovery` skips custom separators
- Fix: create `guard_expr` variant without `:method()` postfix
- Alt: switch to chumsky 0.9+ or winnow
- Test: `match_guard_pass` (currently `#[ignore]`)

---

### L4 — LSP Improvements

**L4-alpha** — Semantic tokens + diagnostics streaming
- `textDocument/semanticTokens/full` support
- Stream parse errors as `textDocument/publishDiagnostics` notifications
- Hover shows function signatures
- Docs: `docs/Tooling.md` L4-alpha section

---

### N2 — Cranelift JIT

**N2-beta** — Cranelift JIT prototype
- `crates/pblang-cranelift/`: compile numeric functions to native code
- Fallback to bytecode interpreter for dynamic features
- Benchmark: compare JIT vs interpreter for fibonacci(40)
- Initial focus: OpAdd, OpSub, OpMul, OpDiv, OpCall for f64 paths

---

### M4 — Registry

**M4-gamma** — Registry token refresh + publish retry (already in vm, needs test)
- Verify `publish_package` 401-refresh flow works end-to-end
- Add hosted registry documentation

---

### R3 — Reverse Engineering

**R3-gamma** — CFG dump verification
- Verify `pblang --cfg <file>` works end-to-end
- Check `tests/cfg.rs` tests pass consistently

---

## Deferred

| Item | Reason |
|------|--------|
| Async runtime | Not planned; VM-per-thread sufficient |
| OS building / bare-metal | Long-term; requires Cranelift first |
| Full HM type inference | Months; design needed |
| Debugger / profiler | Post v1.0 |
| WASM target | Post Cranelift JIT |
