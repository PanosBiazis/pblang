# pblang Roadmap

## Current: 0.6.0, alpha channel — working toward 1.0.0

Updated 2026-09-06. Version/channel come from `pblang.toml`; neither the
presence of a feature nor a passing focused test establishes production readiness.
The older schedules and checkboxes below are historical planning notes, not
current completion evidence. No release date or automatic version bump is set.

### Required order

1. Fix reproducible correctness/data-preservation defects (including diagnostic
   suggestions and autofix), with negative and compatibility regressions.
2. Close native safety and resource boundaries: exact FFI ABI enforcement/raw
   bypass closure, bytecode admission, hard heap/preemption and isolation.
3. Freeze the supported language, bytecode and stable-library contracts; define
   migration/deprecation rules. Keep preview modules explicitly separate.
4. Measure startup, compilation, execution, memory and latency on identical
   workloads before optimizing; preserve correctness and deterministic fallbacks.
5. Verify each supported OS/architecture/backend independently; distinguish
   metadata models, emulators, hosted VMs and physical device evidence.
6. Qualify the release candidate using the compatibility gate below, and publish
   only with verified operator trust, artifact identity and rollback policy.

### Mandatory final compatibility gate before 1.0.0

The final release must check existing applications, PBLang products, examples
and historical source against the candidate, not just the language's unit tests.

- Create an explicit, reviewed inventory of accessible code/projects. Record
  excluded, unavailable, private or unsupported cases separately; untested is
  not passing. Do not silently scan private data or execute discovered code.
- Bind each corpus snapshot and test result to the exact candidate and baseline
  artifacts. Compare the 0.6.0 baseline plus available supported interim releases;
  retain baseline defects as defects, not new regressions or required behavior.
- Check parsing, typing, compilation, runtime results/errors, public APIs,
  imports/packages, file-format round trips, install/upgrade and persistence.
  Use disposable fixtures/copies, never live user documents or databases.
- Exercise CLI, GUI, web and mobile surfaces where implemented, plus each
  independently implemented polyglot adapter and supported emulator/backend.
  Unsupported targets remain explicit, not silently substituted.
- Include focused security, malformed-input, cleanup/cancellation and resource
  tests plus scheduled broader regression/soak coverage on isolated release hosts.
  Daily local work stays targeted; do not run all suites on connected user devices.
- Record every incompatible change with severity, migration guidance and exact
  evidence. No unexplained release-blocking regression may remain. Security fixes
  need not preserve unsafe behavior, but must document the compatibility impact.
- Require a reviewed report listing passed, failed, skipped, unavailable and
  not-tested cases. Do not claim compatibility with all possible programs/devices.

This gate is a release requirement, not a background automation or proof that it
has already run. Current evidence and blockers live in
[`docs/CURRENT_STATUS.md`](docs/CURRENT_STATUS.md),
[`docs/LOW_LEVEL_REVIEW_2026-09-06.md`](docs/LOW_LEVEL_REVIEW_2026-09-06.md) and
[`docs/DIAGNOSTICS_AUTOFIX.md`](docs/DIAGNOSTICS_AUTOFIX.md).

---

## Historical v0.5 planning notes — not current readiness claims

The former plan described the core as production-quality. That statement is
superseded by the explicit current blockers above; it is not a safety guarantee.

**Historical test count only**: 160 passed, 2 ignored. This is not a current run
or an inventory of the current suite.

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
| Historical `env_get` policy | Review required | Environment reads can expose secrets; do not treat them as always safe or infer current capability enforcement from this historical table. |

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
