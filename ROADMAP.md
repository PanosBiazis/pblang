# pblang Roadmap

## Current: v0.5 — "Powerful Prototype / Early-Stage Platform"

The core VM, package manager, and CLI are production-quality. The type system and
native executable bundler are in early stages.

**Current test status**: 83/84 semantics tests passing (1 known parser limitation).

---

## v0.6 — Executables & TLS (Target: +4 weeks)

- [ ] Self-extracting native binaries (Linux + Windows) — E5-gamma
- [ ] TLS/HTTPS builtins (`tcp_listen_tls`, `tcp_connect_tls`) via rustls — W4-gamma
- [ ] `tcp_listen_tls` + `tcp_connect_tls` wired in VM; `stdlib/tls.sapp` upgraded
- [ ] All 84 semantics tests passing (match guard separator fix)
- [ ] Performance benchmarks documented

## v0.7 — Language Maturity (Target: +8 weeks)

- [ ] OOP: class constructor dispatch, method resolution with `self`
- [ ] Match guard separator disambiguation (SP1-gamma — parser rewrite)
- [ ] LSP: semantic tokens (`textDocument/semanticTokens/full`) + diagnostics streaming
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
- ✅ 34+ stdlib modules (HTTP, WebSocket, SQLite, crypto, threads, quantum, AI, tensor)
- ✅ Threading: VM-per-thread + channels + select + thread_pool
- ✅ CLI toolchain: run, test, fmt, lint, build, install, publish, search, bundle
- ✅ Type checker (basic inference for primitives, arrays, maps)
- ✅ LSP server (completions, hover stubs, workspace symbols)
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

1. TLS/HTTPS server (W4-gamma — production blocker)
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
