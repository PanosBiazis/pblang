# pblang Roadmap

## Current: v0.5 — "Powerful Prototype / Early-Stage Platform"

The core VM, package manager, and CLI are production-quality. The type system and
native executable bundler are in early stages.

**Current test status**: semantics suite **160 passed, 2 ignored** (see `docs/CURRENT_STATUS.md` for details).

---

## v0.6 — Executables & TLS (Target: +4 weeks)

- [x] Self-extracting native binaries (Linux + Windows in progress) — E5-gamma
- [x] TLS/HTTPS builtins (`tcp_listen_tls`, `tcp_connect_tls`) via rustls — W4-gamma
- [x] `tcp_listen_tls` + `tcp_connect_tls` wired in VM; `stdlib/tls.sapp` upgraded
- [ ] Semantics suite fully green with 0 ignored tests (match guard separator fix pending)
- [ ] Performance benchmarks documented

## v0.7 — Language Maturity (Target: +8 weeks)

- [ ] OOP: class constructor dispatch, method resolution with `self`
- [ ] Match guard separator disambiguation (SP1-gamma — parser rewrite)
- [x] LSP: semantic tokens (`textDocument/semanticTokens/full`) + diagnostics streaming
- [ ] `pblang new <name> --template web|lib|cli|ai|quantum` scaffolding templates
- [ ] Thread pool stdlib with deadlock diagnostics
- [ ] Cranelift JIT prototype for numeric hot paths (N2-beta)

## v0.8 — Ecosystem (Target: +12 weeks)

- [ ] Registry token refresh & publish retry (M4-gamma)
- [ ] Polyglot block execution bridge (Python/Node subprocess — PL2)
- [ ] VS Code extension: full syntax highlighting + LSP integration
- [ ] AI tool-call example (OpenAI function-calling pattern)
- [ ] Expanded quantum: noise models, Bloch sphere visualization
- [ ] WASM compilation target research

## v0.9 — Production Hardening (Target: +16 weeks)

- [ ] Security audit of sandbox/capability system
- [ ] Native single-binary Stage C (Cranelift/LLVM research)
- [ ] Comprehensive error messages with suggestions ("did you mean...?")
- [ ] Stability policy + deprecation process
- [ ] CI: full matrix (Linux, macOS, Windows) + cargo-audit in CI ✅

## v1.0 — Production Release

- [ ] Production-ready embedded scripting (with full sandboxing + TLS)
- [ ] Production-ready web server deployment (with TLS)
- [ ] Complete package ecosystem (publish, search, install, hosted registry)
- [ ] Native executable distribution (single binary)
- [ ] Comprehensive documentation and tutorials
- [ ] Security audit completed
- [ ] Performance on par with Lua/Wren class interpreters

---

## Already Shipped (v0.5)

- ✅ Core VM (bytecode, GC, sandboxing, error handling)
- ✅ Package manager (lockfile, semver, SHA256, HMAC verify, registry)
- ✅ 100+ stdlib modules (HTTP, WebSocket, SQLite, crypto, threads, quantum, AI, tensor, tooling)
- ✅ Threading: VM-per-thread + channels + select + thread_pool
- ✅ CLI toolchain: run, test, fmt, lint, build, install, publish, search, bundle
- ✅ Type checker (basic inference for primitives, arrays, maps)
- ✅ LSP server (diagnostics, completions, hover, go-to-definition, workspace symbols, semantic tokens)
- ✅ Graphics backend (SDL2 / minifb)
- ✅ Polyglot blocks (@python, @sql, @rust source extraction)
- ✅ VS Code extension (syntax highlighting, tmGrammar)
- ✅ cargo-audit in CI (security scanning)
- ✅ MIT License (filled in)
- ✅ All Clippy warnings resolved
- ✅ CompileError type (panics replaced with proper errors)
- ✅ **New stdlib v0.5**: `result.sapp`, `option.sapp`, `iter.sapp`, `event.sapp`, `cache.sapp`, `validate.sapp`, `uuid.sapp`
- ✅ **Improved stdlib v0.5**: `strings.sapp` (+15 functions), `time.sapp` (+stopwatch, duration), `http.sapp` (+retry, query params), `os.sapp` (+platform, home_dir, which)

---

## Long-Term Vision (Post v1.0)

- **Type System**: Full Hindley-Milner inference, generics, structs/enums
- **Async Runtime**: Optional green threads / async-await
- **JIT Compilation**: Cranelift-based JIT for hot paths
- **WASM Target**: Compile pblang to WebAssembly
- **Debugger**: Step-through debugging with breakpoints
- **Profiler**: Built-in CPU and memory profiling
- **Package Registry**: Hosted public registry (pblang.dev)
- **Self-hosting**: Parser and compiler rewritten in pblang
- **OS scripting**: Userland tooling and build system use cases

---

## Priority Order

1. TLS/HTTPS server (W4-gamma — production release gate)
2. Self-extracting binary (E5-gamma)
3. Match guard fix (SP1-gamma — parser improvement)
4. LSP semantic tokens (L4-alpha)
5. AI/Quantum depth
6. Cranelift JIT (N2-beta)
7. Registry ecosystem
8. Reverse engineering tools
9. Spec/Production hardening
10. Editor support
11. Project scaffolding templates
12. Native/OS integration

---

## 🔐 Security Audit & Fixes (v0.5.1)

### Critical Fixes Applied
| Issue | Severity | Status |
|-------|----------|--------|
| TLS `DangerousAllowAllVerifier` — accepted ANY certificate | 🔴 Critical | ✅ **Fixed** — replaced with `webpki-roots` + `rustls-native-certs` proper CA chain |
| Path traversal in `static_file_response()` | 🟠 High | ✅ **Fixed** — `_path_safe()` strips `../`, URL-decodes, returns 403 on traversal |
| Command injection in `os.which()` | 🟡 Medium | ✅ **Fixed** — new `os_which` builtin searches PATH entries directly (no shell) |
| `fs.sapp` zero input validation | 🟠 High | ✅ **Fixed** — absolute path and `../` denied by default; opt-in `*_abs()` variants |
| URL query parameters not decoded | 🟡 Medium | ✅ **Fixed** — `_parse_query` now calls `url_decode()` on keys and values |
| `NonNull` (T!) type constraint erased in typechecker | 🟡 Medium | ✅ **Fixed** — `Ty::NonNull` preserved; nil assignments to T! now emit diagnostics |
| `http_response` missing 50+ status codes | 🟡 Medium | ✅ **Fixed** — full RFC 7231 reason phrase table via `http_status_reason()` |
| `serve_once_timeout()` ignored timeout_ms | 🟡 Low | ✅ **Fixed** — now passes timeout intent through |
| MIME types missing charset and modern types | 🟡 Low | ✅ **Fixed** — 30+ MIME types with proper charset annotations |
| `env_get` gated behind exec capability | 🟡 Low | ✅ **Fixed** — env reads are always safe; exec gate removed from `env_get` |

### New Standard Library Modules
| Module | Description |
|--------|-------------|
| `testing.sapp` | Full Jest-style test framework: `describe/it/expect/run_all` |
| `orm.sapp` | ActiveRecord-style ORM backed by SQLite |
| `async.sapp` | Go-style goroutines, `await`, `pool`, `timeout`, `debounce`, `throttle` |
| `env.sapp` | `.env` file loading, typed env vars (int/bool/list), schema validation |
| `log.sapp` | Structured leveled logging (text/JSON), named loggers, `timed()` |
| `jwt.sapp` | HS256 JWT sign/verify/decode with expiry and nbf checking |
| `template.sapp` | `{{ var }}`, `{% if %}`, `{% for %}` template engine |

### New Builtins
| Builtin | Description |
|---------|-------------|
| `os_which(cmd)` | Safe PATH search without shell injection |
| `str_from_char_code(n)` | Alias for `strfromcharcode` (used by stdlib) |
| Full HTTP status codes | `http_status_reason(code)` covers all 60+ RFC 7231 codes |
