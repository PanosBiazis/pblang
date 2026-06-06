# pblang Active Backlog

Active tasks only. History belongs in `docs/Report.md`.

**Goal**: pblang evolves as a production platform — embedded + standalone,
AI/Quantum/Web/Polyglot-ready, future-proof, no artificial limits, test-driven
steps, stable core.

**Current priority order:**
TLS/HTTPS ✅ → E5 Executables ✅ → Match Guard ✅ (resolved) → LSP ✅ → Cranelift JIT partial ✅ → Registry ✅ → RE ✅ → Type System ✅ → Spec → Self-Hosting ✅ (tokenizer, parser, type_checker, codegen complete) → Editor → Framework/OS

**Current status**: `.sapp` corpus 160 passed / 0 failed / 0 ignored. The lone ignored test is `os_env_set_pass` in the cargo harness (`env_set` disabled in the v0.5 security audit) — `match_guard_pass` is **no longer ignored**, it passes. Canonical status lives in `docs/CURRENT_STATUS.md`; run `pblang test tests/semantics --allow-all` for the live count (the prebuilt-binary runner, no recompile), or `cargo test --test semantics` for the Rust harness.

---

## ✅ Recently Completed (v0.5 → v0.6)

- **W4 TLS builtins**: `tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls` fully working with rustls
- **E5-gamma**: `crates/pblang-stub` self-extracting binary launcher
- **SP1 Match guards ✅**: Parser restructured; chumsky cascade worked around with `.or_not()` guard sub-parsers — `match_guard_pass` + colon/tilde-separator tests pass (see SP1 section below)
- **L4-alpha**: LSP semantic tokens + hover with builtin signatures
- **N2-beta**: `crates/pblang-cranelift` Cranelift JIT prototype — numeric straight-line/forward-control-flow paths compile to native code; calls, recursion, globals, closures, collections, and dynamic features fall back to the VM
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
- **SP1 Match guards ✅ RESOLVED**: Parser restructured (guarded/unguarded arms split), both `:` and `~` separators accepted; the chumsky 0.8 `parse_recovery` cascade was worked around (`.or_not()` guard sub-parsers); `BindGuard` compiler+VM correct; `match_guard_pass` + separator-variant tests pass
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

### N2 — Cranelift JIT Expansion

**N2-beta** — Cranelift JIT prototype ✅ / expansion open
- `crates/pblang-cranelift/`: compile numeric functions to native code
- Fallback to bytecode interpreter for dynamic features
- Current unsupported surface: `OpCall`, `OpTailCall`, global reads/writes, closures/upvalues, collections, attrs/indexing, try/catch, defer
- Next focus: explicit backend support matrix, then direct calls for simple non-closure functions before recursion benchmarks

---

### SP1 — Match Guard Parser Fix ✅ RESOLVED

- Original blocker: chumsky 0.8 `parse_recovery` cascade at separator chars
  (`cmp_op.padded().repeated()`) — see [[feedback_chumsky_cascade]]. Worked around
  with `.or_not()` guard sub-parsers rather than a wholesale parser-library swap.
- Compiler + VM (`BindGuard`) were always correct; the parser now accepts guarded
  arms with both the `:` and `~` separators.
- Tests: `match_guard_pass`, `match_guard_accepts_colon_separator`, and
  `match_guard_still_accepts_tilde_separator` all **pass** (no longer `#[ignore]`).
  The suite's only ignored test is `os_env_set_pass` (security policy).

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
| OS building / bare-metal | Long-term; requires freestanding codegen/runtime, not just JIT |
| Full HM type inference | Months; design needed |
| Debugger / profiler | Post v1.0 |
| WASM target | Numeric/string subset exists; broaden after backend support matrix |
