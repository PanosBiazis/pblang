# pblang Active Backlog

Active tasks only. History belongs in `docs/Report.md`.

**Goal**: pblang evolves as a production platform — embedded + standalone,
AI/Quantum/Web/Polyglot-ready, future-proof, no artificial limits, test-driven
steps, stable core.

**Current priority order:**
TLS/HTTPS ✅ → E5 Executables ✅ → Match Guard (blocked: parser recovery) → LSP ✅ → Cranelift JIT ✅ → Registry ✅ → RE ✅ → Type System ✅ → Spec → Self-Hosting ✅ (tokenizer, parser, type_checker, codegen complete) → Editor → Framework/OS

**Current status**: 147/149 semantics tests passing, 0 failing, 2 ignored (`match_guard_pass`, `os_env_set_pass`)

---

## ✅ Recently Completed (v0.5 → v0.6)

- **W4 TLS builtins**: `tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls` fully working with rustls
- **E5-gamma**: `crates/pblang-stub` self-extracting binary launcher
- **SP1-gamma PARTIAL**: Parser restructured; blocked by chumsky 0.8 recovery cascade
- **L4-alpha**: LSP semantic tokens + hover with builtin signatures
- **N2-beta**: `crates/pblang-cranelift` Cranelift JIT — fibonacci(20)=6765 in native code, benchmark harness
- **M4-gamma**: Registry 401-refresh flow verified (test passes)
- **R3-gamma**: CFG dump tests verified (`pblang --cfg` works end-to-end)
- **M4-gamma**: Hosted publish flow now has real 401→refresh→retry HTTP unit test in `pblang-pm`
- **T5-delta**: `stdlib/thread.sapp` thread-pool now wraps task messages, preserving `nil` tasks and avoiding fixed-count receive hangs
- **W4-gamma**: `stdlib/websocket.sapp` now supports parse/write for short + 16-bit + 64-bit frame lengths; added masked roundtrip + extended-length tests
- **W4-delta**: Added websocket stream utilities `parse_frames`, `reassemble_fragments`, `parse_message` with continuation-frame (`FIN=0`, opcode `0`) reassembly test
- **W4-epsilon**: VM builtins `websocket_write_frame` / `websocket_parse_frame` now support extended lengths and expose payload/frame metadata; fixed write builtin arity contract to 2 args
- **New stdlib**: `logger.sapp`, `assert.sapp`, `pipe.sapp` (chain, compose, memoize, map/filter/reduce)
- **Typechecker**: nil-safety warnings, division-by-zero detection, better type mismatch messages
- **parse_with_polyglot**: now prefers strict `parse()` over recovery for cleaner error messages
- **for-in loop syntax**: `for x in arr { }` and `for k, v in map { }` — parser + compiler
- **Regex VM builtins**: `regex_match`, `regex_find`, `regex_find_all`, `regex_replace`, `regex_replace_all`, `regex_split`, `regex_capture`
- **str_format builtin**: `%s %d %f %v %%` Python-style formatting
- **New string builtins**: `str_repeat`, `str_pad_left`, `str_pad_right` (str_upper/str_lower already existed)
- **New array builtins**: `array_unique`, `array_zip`, `array_chunk`, `array_flatten`, `array_take`, `array_drop`
- **New stdlib modules**: `regex.sapp`, `config.sapp`, `signal.sapp` (pub-sub)
- **map.sapp extended**: `size`, `is_empty`, `pick`, `omit`, `invert`, `zip_map`, `group_by`, `count_by`
- **LSP completions**: `textDocument/completion` with builtins, keywords, and local declarations
- **LSP hover expanded**: 50+ builtin signatures

- **Security**: MIT License filled in, `cargo-audit` added to CI, all Clippy warnings fixed
- **Error handling**: `CompileError` type, `compile()` returns `Result<Chunk, CompileError>` everywhere
- **Safety**: `unsafe` FFI functions properly marked, SAFETY comments added
- **Code quality**: Zero warnings, `Default` impl for VM, type aliases for complex types
- **New stdlib modules**: `result.sapp`, `option.sapp`, `iter.sapp`, `event.sapp`, `cache.sapp`, `validate.sapp`, `uuid.sapp`
- **Improved stdlib**: `strings.sapp` (+reverse, camelCase, snakeCase, format, lstrip, rstrip, index_of, is_numeric), `time.sapp` (+stopwatch, duration constants, format_duration_ms), `http.sapp` (+retry, build_query_string, get_auth, post_json_auth), `os.sapp` (+platform, home_dir, temp_dir, which, getenv, require_env)
- **tls.sapp**: Upgraded from pure stub to real dispatch to `tcp_listen_tls`/`tcp_connect_tls` builtins
- **rustls + rustls-pemfile**: Added to `pblang-vm/Cargo.toml` — TLS builtin implementation ready
- **W4 TLS builtins IMPLEMENTED**: `tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls` fully working with rustls
- **E5-gamma IMPLEMENTED**: `crates/pblang-stub` self-extracting binary launcher with tar.gz extraction, cleanup, --extract-only, --bundle-version flags
- **SP1-gamma PARTIAL**: Parser restructured (guarded/unguarded arms split), `~` separator defined; blocked by chumsky 0.8 `parse_recovery` cascade limitation; `BindGuard` compiler+VM fully correct
- **L4-alpha IMPLEMENTED**: LSP semantic tokens (`textDocument/semanticTokens/full`) + hover with builtin signatures and local function docs

---


## ✅ v0.5.4 — Recent Completions

- **benchmark.sapp**: Restored with backward-compatible API (run, time, warmup, compare)
- **compression.sapp**: Gzip, Zstd, Brotli compress/decompress + auto-detect
- **serialization.sapp**: JSON pretty/validate, TOML/YAML, CSV, XML escape
- **bitops.sapp**: 26 bit-level operations (AND/OR/XOR/NOT, shifts, popcount, CLZ/CTZ, rotations)
- **registry_server.sapp**: Hosted package registry HTTP server (GET /packages, /search, /publish)
- **compiler/codegen.sapp**: Self-hosting bytecode code generator (OpCode constants, chunk building, expr/stmt compilation)
- **String constant folding**: Compile-time string concatenation optimization
- **Peephole optimizer**: Dup+Pop, Not+Not removal
- **Cranelift fix**: Added missing min_arity/defaults/is_vararg fields
- **Syntax fixes**: All stdlib modules use proper pblang syntax (map{}, len(), array_push(), etc.)


## Pending Tasks

### N2 — Cranelift JIT (highest priority)

**N2-beta** — Cranelift JIT prototype
- `crates/pblang-cranelift/`: compile numeric functions to native code
- Fallback to bytecode interpreter for dynamic features
- Benchmark: compare JIT vs interpreter for fibonacci(40)
- Initial focus: OpAdd, OpSub, OpMul, OpDiv, OpCall for f64 paths

---

### SP1 — Match Guard Parser Fix (blocked)

**SP1-gamma** — chumsky 0.8 `parse_recovery` cascade limitation
- Root cause: `cmp_op.padded().repeated()` generates error cascade at separator chars
- Fix: migrate to chumsky 0.9+ (improved recovery model) or use winnow/nom
- Compiler + VM: fully correct (`BindGuard` works end-to-end)
- Parser: arms split into `guarded_match_arm` / `unguarded_match_arm` (structurally correct)
- Test: `match_guard_pass` stays `#[ignore]` until parser migration

---

### M4 — Registry

**M4-gamma** — Registry token refresh + publish retry ✅
- End-to-end retry path verified with local HTTP test server:
  first `/publish` returns 401, `/auth/refresh` returns new token, second `/publish` succeeds.
- Hosted registry docs remain incremental work as API surface expands.


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
