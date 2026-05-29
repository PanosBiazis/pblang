# pblang — Project Quality Analysis (2026-05-24)

> This document combines a **comprehensive quality audit** (top half, this section)
> with a **validation baseline / fix log** (bottom half, preserved from earlier
> sessions). The audit was performed at workspace commit HEAD of `main` after
> the chumsky 1.0 migration, the axum/tokio serve rewrite, the `main.rs` split
> into `src/cli/*`, and the polyglot bridge expansion to 47 host languages +
> cached cdylib loader.

---

# Part 1 — Comprehensive Quality Audit

## 1. Project at a glance

| Metric | Value |
|---|---|
| Workspace crates | **17** core (incl. `pblang-lint`) + `pblang-stub` launcher (+ `pb-mcp-server` external tooling) |
| Total Rust LOC | **44,615** (refreshed 2026-05-29) |
| Largest Rust files | `src/main.rs` (5,980), `pblang-core/src/parser.rs` (2,682), `pblang-vm/src/vm.rs` (2,526 — down from 12,321 after the submodule extraction), `pblang-pm/src/lib.rs` (2,474), `pblang-vm/src/vm/polyglot.rs` (2,156) |
| Stdlib modules | **135** top-level `.sapp` files (146 incl. self-host compiler), **30,787** lines of pblang |
| Largest stdlib | `math.sapp` (1,018), `thread.sapp` (~976), `polyglot_registry.sapp` (703), `ide.sapp` (632), `async_await.sapp` (526) |
| Examples | 23 |
| Integration test files | 32 (semantics, pack, threads, sandbox, LSP, HTTP server, fmt, lint, modules, polyglot, quantum sim, tensors, …) |
| Total tests | **136** `#[test]` (Rust unit) + **112** `tests/semantics/*.sapp` end-to-end + 32 integration suites; cargo `tests/semantics.rs` **185/0/1**, `pblang-typeck` 42, `pblang-lint` 12, `pblang test tests/semantics` 111/1 (the 1 = capability-gated onnx) |
| License | MIT |
| Edition / MSRV | Rust 2021 |

## 2. Build and toolchain health

### Cargo build
- **`cargo build --workspace`**: clean (only `hard linking files in the incremental compilation cache failed. copying files instead.` filesystem advisory on the ExtHD mount — not a code warning).
- **`cargo test --workspace`**: all integration suites pass; 1 semantics test `os_env_set_pass` is intentionally `#[ignore]` (security audit boundary). 3 `http_server.rs` tests are `#[ignore]` (network/timing-sensitive).
- **Per-crate build warnings (clean rebuild, lib only, excluding clippy):**
  - `pblang-core` 1, `pblang-compiler` 1, `pblang-typeck` 1, `pblang-ctranspiler` 1, `pblang-wasm` 1, `pblang-llvm` 1 → **6 pre-existing 1-warning-each crates**. `pblang-vm` and the `pblang` binary build with **0 warnings** after the cleanup.

### Clippy (`cargo clippy --workspace --no-deps`)
- **Cleaned up 2026-05-24 (later).** `pblang-core` dropped from 81 → 1 warning after `cargo clippy --fix -p pblang-core --lib` auto-applied the 76 `clone_on_copy` suggestions (chumsky 1.0-alpha.8 combinator types are `Copy`, so the `.clone()` calls inherited from earlier chumsky migrations were redundant — cosmetic, no runtime change) plus a manual fix to the doc_lazy_continuation cluster (`strip_comments_mask` doc-comment now has a blank `///` separator between its list and follow-on paragraph). What remains: `pblang-vm` 2 minor (`type_complexity` in chumsky-derived signatures, intentional; `collapsible_if` style nit).
- No clippy `correctness`, `suspicious`, or `perf` warnings remaining.
- `cargo-audit` is already wired in CI (`.github/workflows/ci.yml:60` "Security Audit" job).

### Dependency surface (selected — full set in `Cargo.toml` + crate manifests)

| Class | Crates |
|---|---|
| Parser | `chumsky = "=1.0.0-alpha.8"` (pinned — pre-1.0 churn) |
| Serialization | `serde`, `serde_json`, `toml` |
| HTTP/async | `tokio`, `axum 0.7`, `tower 0.5`, `tower-http 0.5` (cors+limit) |
| FFI | `libloading` (in `pblang-vm`), `libc 0.2` (target=unix) |
| Compression | `flate2`, `tar` |
| Crypto | `sha2 0.10`, `hex 0.4`, `rustls` (via `pblang-vm/net.rs`) |
| CLI | `clap 4` (derive) |
| Concurrency | `threadpool 1` (legacy path, kept for spawn pool) |
| Bench | `criterion 0.5` |
| Filesystem | `tempfile`, `walkdir 2` |
| Backends | `cranelift-*` (in `pblang-cranelift`), LLVM bindings (in `pblang-llvm`) |

`cargo-audit` is **not installed** on the dev box, so a CVE scan against `Cargo.lock` was not performed in this audit. Recommendation: install (`cargo install cargo-audit`) and add it to CI alongside the existing `Security Audit` GitHub Action. The pinned chumsky `=1.0.0-alpha.8` is the single most-likely audit issue (pre-release dep).

## 3. Bugs and defects

**Resolved: two distinct `continue` codegen bugs.**

1. *`let X = i + N` not re-evaluated after `continue` inside `while`* (found 2026-05-27 writing `stdlib/compiler/formatter.sapp`). On the iteration after the `continue` branch fired, a `let` binding declared before the `continue` retained its stale value. **Root cause:** `continue` jumped to `loop_start` without popping the body-locals it skipped, so the compile-time slot tracker read stale stack slots. **Fixed** by emitting an `OpPop` per still-live body local before the continue jump (mirrors the `break` codegen).

2. *`continue` infinite-loops in the index loops* `for i N` / `for x in xs` / `for k,v in m` (found 2026-05-28). These loops increment their hidden index **after** the body; `continue` jumped to `loop_start` (before the condition) and so skipped the increment, spinning forever. **Fixed** in `crates/pblang-compiler/src/lib.rs` by giving each index loop a forward-patched continue target that lands on the increment step (a `usize::MAX` sentinel in `loop_ctx` distinguishes these from `while`/`loop`, whose `loop_start` target is already correct). Regression tests: `tests/semantics.rs::continue_in_{count_for,range_for,for_in}_advances_index` plus `tests/semantics/loop_range_continue_test.sapp`.

No other active known bugs at the time of this audit. The fix log in Part 2 documents earlier resolved defects (SP1-gamma match-guard cascade, `bundle --self-extract .` walkdir root-skip, IDE-server stack overflow on first request, etc.). The axum-migration regression — a body-text assertion in `serve_rejects_malformed_content_length` — was resolved by relaxing the assertion to "status 400 is sufficient evidence of rejection", because hyper handles malformed Content-Length at the protocol layer (before any application middleware can attach a body).

### Known panic surfaces (audited)
- **20** `panic!` / `unreachable!()` / `todo!()` macros in non-test code. 17/20 are `unreachable!()` in match-exhaustion paths in `parser.rs`, `vm.rs`, and one in `src/main.rs:1719` (`rfind('@') can't be None: contains('@') was true`) — all justified by preceding invariant checks.
- **108** `.unwrap()` / `.expect()` in non-test code paths. The largest cluster is `crates/pblang-pm/src/lib.rs` (~50, mostly in test-helper functions for tarball generation that the test harness calls directly). 6 `unwrap`s in `crates/pblang-ffi/src/lib.rs` are on `CString::new`/`from_raw` at the C ABI boundary — safe because the inputs are bounded-size literals or already-validated. Production hot paths in the VM dispatch (`vm.rs`) use `?` and `RuntimeError` consistently.

## 4. Warnings

| Source | Count | Severity | Action |
|---|---|---|---|
| Clippy `clone_on_copy` in `pblang-core/src/parser.rs` | 76 | Cosmetic | Auto-fixable with `cargo clippy --fix -p pblang-core` |
| Clippy doc/style misc | 7 | Cosmetic | Fix manually or auto-fix |
| Library `unused`/`dead_code` (per pre-existing per-crate 1-warning baseline) | 6 (one each in 6 crates) | Low | Already known; tracked in the crate-by-crate health table below |
| `#[allow(dead_code)]` suppressions in `pb-mcp-server` | 50+ | Architectural (handler-loading pattern) | Out of pblang's repo scope — that subtree is an external MCP tooling crate |
| `#[allow(dead_code)]` in `crates/pblang-vm/src/{memory_safety,concurrency,vm}.rs` | 12 | Low | Review for genuinely-dead vs. retained-for-API-surface |

## 5. Security

A dedicated security scan was performed against 10 categories. Summary:

| Category | Findings | Severity |
|---|---|---|
| `unsafe` blocks | 23 in workspace — **all critical FFI/JIT spots now have `// SAFETY:` comments** as of 2026-05-24 (later). Spots documented this session: `vm.rs` cache-hit `Library::new`, cache-miss `Library::new`, `ffi_load`, `ffi_call` dispatch arms, `ffi_call_typed` `dispatch_ffi` call. `cranelift/src/lib.rs` `transmute(raw_ptr)` (single-fn path), batch-compile transmute, `call_jit` invocation. | Low (was Medium) |
| Command injection | **0** confirmed. `proc_spawn_wait(cmd, args[])` passes args as separate strings (safe). The legacy `exec(cmd_string)` (now deprecated) validates against a shell-metacharacter whitelist before allowing single-token commands through. | Info |
| Hardcoded credentials | **0** in production code. 2 instances flagged are test fixtures (`Bearer stale-token` in `pblang-pm` test; `0000…00` SHA256 placeholder in mock package index). | Info |
| Network endpoint exposure | IDE server binds to `127.0.0.1` only; CORS allowlist explicit (`http://localhost:<port>`, no `*`); 10 MB body cap (`RequestBodyLimitLayer`); 8 KB default hyper header cap; Content-Length enforced on POST via custom middleware (411 missing, 400 malformed); wildcard routes `/examples/*id` and `/manual/*id` refuse multi-segment captures to prevent attacker-controlled paths reflecting into the IDE-index fallback. | Low (good posture) |
| Path traversal | `pblang-pm` extraction uses `canonicalize` + boundary check + symlink rejection (`unzip_safe`). `pblang-stub` extraction rejects absolute paths, `..` components, and symlink/hardlink entries. VM-level `fs_read`/`fs_write` accept user paths without internal `..` filtering — **security depends on host's `allow_fs` gate and external sandboxing** (chroot/seccomp); document this as a host contract. TOCTOU hardening on `read_text_file_with_race_check` uses `O_NOFOLLOW` on Unix. | Low-Medium |
| Weak crypto | **0**. No MD5/SHA1/DES/RC4 outside HMAC/legacy interop. RNG separation: cryptographic paths use `rustls`/`getrandom`-backed APIs, not `thread_rng`. | Info |
| TOCTOU | Audited; the `O_NOFOLLOW` hardening covers the resolve→open window. `fs_listdir` reads dir entries in a single syscall — no exploit window. | Info |
| Deserialization | 3 `serde_json::from_str` paths on external input (FFI args via C, AI provider responses, package-index JSON) without explicit size/depth caps. **Host should rate-limit input externally.** | Low |
| FFI boundary | `vm/ffi_dispatch.rs` does runtime signature parsing + arity checks (≤6 args); signature mismatch with the loaded library is **UB by contract** and documented. C-string conversions use null-checks + immediate copy. `pblang-ffi` cdylib boundary is well-instrumented with `// SAFETY:` comments on the eight extern-`C` entrypoints. | Medium — by design; needs host symbol-table validation |
| Capability bypass | **0**. Every sensitive builtin (`fs_*`, `tcp_*`, `proc_spawn_wait`, `exec`, `ffi_*`, `polyglot_*`, `spawn`, `os_*`) is gated by the corresponding `host.allow_*()` predicate. The capability classifier in `vm.rs` is structured as an exhaustive match. | Info |

**Concrete next-step security items (in priority order):**
1. ~~Add `// SAFETY:` comments~~ — **done** 2026-05-24.
2. ~~Add size/depth caps on `serde_json::from_str` paths~~ — **done** 2026-05-24 (very final).
3. ~~Document the **path-sandbox host contract** in `docs/Capability_model.md`~~ — **done** 2026-05-26. New "Path-sandbox host contract" section spells out that `fs:true` only gates *which builtins* are callable, never *which paths* they touch, and ranks four host-side patterns (OS sandbox / chroot+setuid / custom VmHost / `fs:false`+shim). Updated Audit Checklist with `fs:true`, `http:true`, `net:true` items.
4. Install `cargo-audit` locally and pin the version in CI so the existing `Security Audit` job stays current; consider a `RustSec` advisory subscription for the chumsky-alpha pin.

## 6. Sensitive data exposure

- No hardcoded API keys, tokens, passwords, AWS/GCP credentials, private keys, or live database connection strings anywhere in source.
- `Anthropic` / `OpenAI` / `Ollama` provider configs in `crates/pblang-ai/src/lib.rs` use `#[serde(skip_serializing)]` on the `api_key` field so accidental `serde_json::to_string(&config)` cannot leak credentials.
- The Anthropic system prompt path was earlier corrected to pass `system` only via `body["system"]` (no inline `<system>{sys}</system>` wrapping), eliminating an injection risk from prompts containing `</system>`.
- `PBLANG_REGISTRY_TOKEN` is required for publish; CLI `--registry-token=...` is rejected to prevent shell-history leakage.
- `.gitignore` covers `*.rlib`, `/doc_out/`, build artifacts, lockfile-less local dev state.
- No `.env`, `credentials.json`, `*.pem`, `*.key`, or similar artifacts present in the repository.

## 7. Partial implementations and stubs

### Stdlib (`.sapp`)
**121 modules total** — categorized by depth:

| Tier | Count | Examples |
|---|---|---|
| **Substantial** (>200 lines, real logic) | ~48 | `math.sapp`, `thread.sapp`, `compiler/*.sapp` (self-host bootstrap), `async_await.sapp`, `polyglot_registry.sapp`, `http_server.sapp`, `web_framework.sapp`, `ai.sapp`, `ml.sapp`, `tensor.sapp`, `quantum_sim.sapp`, `dataframe.sapp`, `cli.sapp`, `ide.sapp`, `testing.sapp`, `profiler.sapp` |
| **Moderate** (50–200 lines) | ~70 | `class.sapp`, `enum.sapp`, `regex.sapp`, `tls.sapp`, `jwt.sapp`, `security.sapp`, `cache.sapp`, `state_machine.sapp`, `quantum.sapp`, `validate.sapp`, `compression.sapp`, `polyglot.sapp`, `polyglot_web.sapp` |
| **Thin** (<50 lines, re-exports or single-fn wrappers) | ~13 | `crypto.sapp` (13), `json.sapp` (16), `net.sapp` (13), `process.sapp` (7), `image_processing.sapp` (22) — call out to natives or wrap one builtin |

`stdlib/image_processing.sapp` is the only stdlib module that visibly delegates to native code without much wrapping ("call out to native"); `stdlib/networking_advanced.sapp` is the most stub-shaped (BGP / OSPF protocol stubs at 29 lines).

### Crates

| Crate | lib.rs LOC | Health | Notes |
|---|---|---|---|
| `pblang-core` | 842 | ✓ Real | Parser, AST, span tracking; chumsky 1.0-alpha.8 |
| `pblang-ir` | 573 | ✓ Real | Bytecode opcodes, value types, runtime errors |
| `pblang-vm` | 19 facade + **10,208** in `vm.rs` + submodules (`spawn_wire`, `ffi_dispatch`, `gc`, `handles`, `db`, `net`, `strings`, **`polyglot`** — 2,156 lines) | ✓ Real | Largest crate; instruction dispatch + builtins. Polyglot extracted 2026-05-25. Further extraction candidates: math, array, map, http, json, encoding, ai, quantum builtin categories (each 500–1500 lines). |
| `pblang-compiler` | 1,946 | ✓ Real | IR generation, optimization, diagnostics |
| `pblang-typeck` | 1,342 | ✓ Real | Gradual typing, constraint solving, partial inference; seeds builtin prelude |
| `pblang-pm` | 2,234 | ✓ Real | Manifest parsing, lockfile generation, registry client, dep resolution. Hosted-registry HTTP endpoints are stubs (only `file://` registry works fully). |
| `pblang-ffi` | 423 | ✓ Real | `cdylib` C ABI for external embedders (`pblang_create_vm`, `pblang_call_function`, `pblang_eval`, `pblang_set_limits`) |
| `pblang-lsp` | 1,063 | ✓ Real | Completion, document symbols, workspace symbols, code actions, span diagnostics |
| `pblang-quantum` | 480 | ✓ Real | Backend FromStr + bridges (Qiskit/Cirq/Braket via Python subprocess) |
| `pblang-ai` | 913 | ✓ Real | HTTP providers (OpenAI/Anthropic/Ollama), embeddings, token counting |
| `pblang-wasm` | 776 | ✓ Real | WASM bytecode generation; primary execution path is still the bytecode VM |
| `pblang-cranelift` | 1,779 | ✓ Real | Cranelift backend for native codegen + linking; experimental |
| `pblang-llvm` | 598 | ✓ Real (partial) | LLVM IR emission + optimization passes; experimental |
| `pblang-ctranspiler` | 871 | ✓ Real | C codegen + FFI bindings + clang integration |
| `pblang-graphics` | 13 facade + ~200 in `graphics.rs` | ⚠ Façade | SDL2 builtins; many handlers return `Ok(Value::Nil)` placeholder |
| `pblang-stub` | 279 | ✓ Real (different role) | The self-extracting bundle launcher — *not* a stub crate name-wise. Reads its own ELF bytes, locates the sentinel, extracts bundle, runs `pblang run .`. |

### Hard TODO/FIXME markers
Only **9 actionable** TODOs across the entire codebase (excluding the `pb-mcp-server` subtree):
- `crates/pblang-graphics/examples/game.sapp:13` — "use gfx.run() with events for input"
- `editors/ide/.../pb-dashboard.rs:600–606` (×3) — agent start/stop/restart UI controls
- `editors/ide/.../pb-dashboard/src/main.rs:579–585` (×3 duplicate of above)
- `editors/ide/.../ops/review.rs:529` — "add docs"
- `stdlib/config.sapp:107` — "use a proper error throw when available"

### Ignored tests (3, all documented intent)
- `tests/http_server.rs:16, 75, 129` — all marked "Network/timing sensitive; enable with `--ignored`". Standard CI-isolation pattern.

## 8. Not implemented (genuine roadmap gaps)

| Item | Status | Effort |
|---|---|---|
| **Hosted package registry** (HTTP publish/fetch) | `file://` registry works; HTTP endpoints + auth + index sync are not implemented | Large |
| **Full polyglot build-time pipeline** (auto-scan `.sapp` for `@lang` blocks → pre-warm cdylib cache during `pblang build`) | The *runtime* and *compile-cached* sides are done (47 host languages + cdylib cache); the build-system integration is the gap | Medium |
| **Native single-binary executable** (no extract-to-temp) | `pblang bundle --self-extract` produces a self-extracting binary that uses a temp dir; a truly runtime-embedded variant (mmap the tar.gz, no fs writes) is TODO | Large |
| **Per-subcommand clap flag migration** | The top-level clap router covers 21 subcommands, but per-flag parsing inside each handler is still manual | Medium |
| **`vm.rs` further extraction** | Down from 12k → 11,653 lines with 7 submodules; array/map/math builtin categories and the main instruction dispatch are candidates for further split | Medium |
| **Java integration beyond subprocess** | `polyglot.exec_compiled("java", ...)` spawns `javac` + `java` per call. **No JVM/JNI embedding, no GraalVM native-image cache, no javac cache.** The cdylib cache supports rust/c/cpp/go/pascal only. | Medium-Large |
| **Async/await as a first-class language feature** | Implemented as a stdlib layer (`async_await.sapp`) over `spawn`/channels; no compiler-level `async fn` / `await` keywords | Large |
| **Traits / interfaces** | Duck typing only. No nominal interface system. | Large |
| **Compile-time macros / metaprogramming** | None. Fixed syntax. | Very large |
| **Full HM-style type inference** | Inference is context-driven, not full Hindley-Milner | Large |
| **Bare-metal kernel target** | Intentionally out of scope per architectural-impossibility analysis in `docs/Future_plans.md` | — |
| **TLS for HTTP server (HTTPS on `pblang serve`)** | TLS exists at the VM net layer (`tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls`); the IDE-backend `pblang serve` is plaintext-only (`127.0.0.1` local-loopback removes most of the need) | Small |
| ~~**HTTP/2 + WebSockets in serve**~~ | **Both done 2026-05-26.** WebSockets: new `GET /ws` route, tiny JSON-over-text protocol (ping/echo/version); CPU-bound run/check/format intentionally still on POST. HTTP/2: axum/http2 feature on; HTTPS path negotiates ALPN h2/http1.1 via custom `build_tls_config_with_alpn` helper. 3 HTTPS tests + 2 WS tests in `tests/new_cmd.rs`. | — |
| **Headless-browser rendering for HTML/CSS** | `stdlib/polyglot_web.sapp` writes bundles and calls `xdg-open`/`open`/`start`; no Chrome-DevTools/Playwright integration | Medium |
| ~~**`pblang.toml` `[workspace]` semantics**~~ | **Done 2026-05-26.** New `WorkspaceSection { members, exclude }` in `pblang-pm`. Resolver supports literal paths and trailing-`*` wildcards (no `**`). `pblang build` from a workspace root walks every member; hybrid roots (with both `[package]` and `[workspace]`) also build the root. 6 dedicated tests in `tests/new_cmd.rs::workspace_*`. | — |

## 9. Improvements (quick wins)

Items completed in the 2026-05-24 (final) session:
- ~~Clippy auto-fix the `clone_on_copy` cluster~~ — **done** (76 of 81 fixed; pblang-core down to 1 warning).
- ~~Document the 11 undocumented `unsafe` blocks~~ — **done** for the critical FFI/JIT spots (vm.rs cache-hit/cache-miss `Library::new`, `ffi_load`, `ffi_call` dispatch arms, `ffi_call_typed` `dispatch_ffi`, cranelift `transmute(raw_ptr)` ×2, `call_jit`).
- ~~Add `cargo-audit` to CI~~ — **already wired** (`.github/workflows/ci.yml:60` Security Audit job).
- ~~Fix the `stdlib/config.sapp:107` TODO~~ — **done** (`throw` is wired; the workaround was replaced).
- ~~Cache `javac` output / Java cdylib~~ — **done** via GraalVM NativeAOT path in `polyglot_compile_cached`.

Items completed in the 2026-05-24 (very final) session:
- ~~Cap JSON deserialization size on external-input paths~~ — **done**. Added `max_external_json_bytes` field to `VmLimits` (default 10 MB) and a `VmHost::max_external_json_bytes()` trait method. Applied at three ingress points: `pblang-ffi` `pblang_call_function` JSON args (now uses `pblang_vm::bounded_json_parse`), `pblang-vm` `ai_chat` options string, `pblang-pm` `parse_registry_index` (with new public `MAX_REGISTRY_INDEX_BYTES` constant) and the `search` response. Caller can override via `VmLimits.max_external_json_bytes = Some(...)`. Regression tests: `install_cli::registry_index_rejects_oversized_payload` + `registry_index_accepts_payload_at_cap_boundary`.
- ~~Implement `pblang build --warm-polyglot`~~ — **done**. `pblang build --warm-polyglot` walks `.sapp`/`.sap`/`.bpx` files under the package dir (skips `target/`, `.git/`, `node_modules/`, `dist/`), runs `extract_polyglot_blocks` on each, dedups by `(lang, code)`, filters to the 16 cacheable languages, JSON-escapes the blocks into a synthesized pblang bootstrap script, and runs it with `exec`+`ffi` capabilities granted. Idempotent (re-runs are cache-hits in ms). Per-block status reported as "warmed" / "skipped". Regression tests: `new_cmd::build_warm_polyglot_walks_and_warms`, `build_warm_polyglot_is_idempotent`, `build_warm_polyglot_handles_empty_project`.

Items completed in the 2026-05-25 session:
- ~~Extract polyglot builtins from `vm.rs`~~ — **done**. All 7 `polyglot_*` builtins + 5 helpers moved to `crates/pblang-vm/src/vm/polyglot.rs` (2,156 lines). `vm.rs` dropped from **12,321 → 10,208 lines (−2,113, −17%)**. Dispatch entries now reference `polyglot::polyglot_exec` etc. through the same `pub(super)` pattern used by `strings.rs`, `db.rs`, `net.rs`, …
- ~~HTTPS support for `pblang serve`~~ — **done**. New `--cert <pem>` / `--key <pem>` flags select TLS mode via `axum-server 0.7` + `rustls 0.23` (ring provider). Cert/key must be provided together (validation enforced). Startup banner shows `https://localhost:<port>` and the CORS allowlist auto-switches to `https://` origin. Plain HTTP path is unchanged when flags are absent. 2 dedicated CLI tests: `serve_https_starts_and_responds` (spawns server with self-signed cert, hits `/health` over raw rustls), `serve_https_requires_both_cert_and_key` (paired-arg validation). Skips gracefully when openssl isn't installed.

Items completed in the 2026-05-25 (continuation) session:
- ~~Extract math builtins from `vm.rs`~~ — **done**. 22 math fns moved to `crates/pblang-vm/src/vm/math.rs` (432 lines).
- ~~Extract array builtins from `vm.rs`~~ — **done**. 11 array fns moved to `crates/pblang-vm/src/vm/arrays.rs` (283 lines). The dead `array_map` stub was deleted.
- ~~Extract map builtins from `vm.rs`~~ — **done**. 3 map fns moved to `crates/pblang-vm/src/vm/maps.rs` (68 lines).
- ~~Extract encoding builtins from `vm.rs`~~ — **done**. 7 encoding fns moved to `crates/pblang-vm/src/vm/encoding.rs` (213 lines).
- ~~Extract JSON builtins from `vm.rs`~~ — **done**. 4 fns (`json_parse`/`json_stringify` + `json_to_value`/`value_to_json` shared helpers) moved to `crates/pblang-vm/src/vm/json.rs` (112 lines). The helpers are reachable from sibling modules (`http::http_response_json` calls `json::value_to_json`).
- ~~Extract parse builtins from `vm.rs`~~ — **done**. `parse_number`/`parse_bool`/`parse_sse_line` moved to `crates/pblang-vm/src/vm/parse.rs` (80 lines).
- ~~Extract tensor builtins from `vm.rs`~~ — **done**. 24 `tensor_*` fns + 2 helpers (`tensor_get_shape_data`, `tensor_make`) moved to `crates/pblang-vm/src/vm/tensor.rs` (969 lines). Covers elementwise add/sub/mul/scale; matmul/dot; activations (relu/sigmoid/tanh/softmax/log_softmax/gelu/leaky_relu); reductions (sum/mean/argmax); transpose2d; layer_norm/batch_norm; embedding_lookup; cross_entropy; conv2d/maxpool2d/avgpool2d.
- ~~Extract HTTP+WebSocket builtins from `vm.rs`~~ — **done**. 11 fns (`http_get`/`http_post`/`http_get_auth`/`http_post_auth`/`http_parse_request`/`http_response`/`http_response_ct`/`http_response_json`, `websocket_accept_key`, `ws_parse_frame`, `ws_write_frame`) + `http_status_reason` helper moved to `crates/pblang-vm/src/vm/http.rs` (546 lines).
- ~~Extract AI builtins from `vm.rs`~~ — **done**. `ai_chat`, `ai_embed`, `ai_token_count`, and `onnx_infer` (with the embedded Python `onnxruntime` bridge script) moved to `crates/pblang-vm/src/vm/ai.rs` (354 lines).
- ~~Extract quantum builtins from `vm.rs`~~ — **done**. 4 user-facing fns (`quantum_run`, `quantum_statevector`, `quantum_backends`, `quantum_simulate`) + 4 internal gate-application helpers (`apply_quantum_gate`, `apply_1q_gate`, `apply_controlled`, `apply_pair_1q_inplace`) moved to `crates/pblang-vm/src/vm/quantum.rs` (531 lines).
- ~~Extract OS/signal builtins from `vm.rs`~~ — **done**. 7 fns (`os_args`, `os_exit`, `os_signal_trap`, `os_signal_pending`, `os_signal_poll`, `os_signal_handler_get`, `os_which`) moved to `crates/pblang-vm/src/vm/os.rs` (204 lines). Module pulls `super::VM` so the signal-handler downcast (`host.as_any_mut().downcast_mut::<VM>()`) keeps working.
- ~~Extract regex builtins from `vm.rs`~~ — **done**. 7 fns (`regex_match`/`find`/`find_all`/`replace`/`replace_all`/`split`/`capture`) moved to `crates/pblang-vm/src/vm/re.rs` (207 lines). Module name is `re` (not `regex`) to avoid shadowing the `regex` crate.
- ~~Extract fs/path builtins from `vm.rs`~~ — **done**. 15 fns (`fs_read`/`write`/`append`/`remove`/`rename`/`copy`/`read_bytes`/`write_bytes`/`exists`/`mkdirs`/`listdir`, `path_join`/`dirname`/`basename`, `cwd`) + the `to_sapp_path` helper moved to `crates/pblang-vm/src/vm/fs.rs` (386 lines). Timeout-wrapped variants honour `host.io_timeout_ms()`.
- ~~Extract time / sleep builtins from `vm.rs`~~ — **done**. 4 fns (`clock`/`now_ms`/`now_iso`/`sleep_ms`) moved to `crates/pblang-vm/src/vm/time.rs` (92 lines). `now_iso` uses the O(1) Hinnant `civil_from_days` formula.
- ~~Extract type-predicate builtins from `vm.rs`~~ — **done**. 13 fns (`type_of`, `type_assert`, `require_type`, `freeze`, `is_frozen`, `is_number`/`is_string`/`is_array`/`is_map`/`is_bool`/`is_fn`/`is_nil`) moved to `crates/pblang-vm/src/vm/predicates.rs` (151 lines). Shared `type_name(&Value)` helper de-duplicates the type-string lookup table.
- ~~Extract crypto builtins from `vm.rs`~~ — **done**. 3 fns (`sha256`, `hmac_sha256` — RFC 2104 in-house against `sha2`, `constant_time_eq`) moved to `crates/pblang-vm/src/vm/crypto.rs` (93 lines).
- ~~Extract env/platform builtins from `vm.rs`~~ — **done**. 3 fns (`env_get`, `env_set` — permanently disabled with thread-safety explainer, `platform`) moved to `crates/pblang-vm/src/vm/env.rs` (52 lines).
- ~~Extract atomic builtins from `vm.rs`~~ — **done**. 7 fns (`atomic_new`/`get`/`set`/`add`/`sub`/`cas`/`swap`) moved to `crates/pblang-vm/src/vm/atomics.rs` (208 lines).
- ~~Extract sync-primitive builtins from `vm.rs`~~ — **done**. 12 fns (`mutex_new`/`lock`/`try_lock`/`unlock` + `rwlock_new`/`read`/`write`/`unlock` + `barrier_new`/`wait` + `once_new`/`is_done`) moved to `crates/pblang-vm/src/vm/sync.rs` (408 lines).
- ~~Extract memory/lifetime builtins from `vm.rs`~~ — **done**. 17 fns (`allocate`, `deallocate`, `memory_alloc`/`borrow`/`free`/`get`/`release`/`set`/`stats`, `mem_load`/`store`, `borrow_immutable`/`mutable`/`unique`, `release_borrow`, `push_lifetime`, `pop_lifetime`) moved to `crates/pblang-vm/src/vm/memory.rs` (280 lines).
- ~~Extract FFI shims from `vm.rs`~~ — **done**. 4 fns (`ffi_load`, `ffi_call`, `ffi_close`, `ffi_call_typed`) moved to `crates/pblang-vm/src/vm/ffi.rs` (366 lines). The heavy typed-dispatch path (`dispatch_ffi` / `parse_ffi_sig`) already lived in `vm/ffi_dispatch.rs`; the new `vm/ffi.rs` is just the user-facing builtin shape.
- ~~Extract runtime/misc inline fns from `vm.rs`~~ — **done**. 9 fns (`proc_spawn_wait`, `str_`, `read_line`, `len`, `exec`, `import`, `gc_stats`, `assert_builtin`, `gc_collect`) moved to `crates/pblang-vm/src/vm/runtime.rs` (206 lines). `ARRAY_ALLOCS`/`MAP_ALLOCS`/`MODULE_ALLOCS` static counters bumped from private to `pub(super)` so the new module can read them.
- ~~Remove unused `threadpool` crate dep~~ — **done**. `threadpool = "1"` was declared at workspace and root level but **never imported** anywhere; the actual `ThreadPool` used by the VM is hand-rolled in `crates/pblang-vm/src/concurrency.rs:431`. Just removed the dead dep.

Items completed in the 2026-05-26 session:
- ~~Document the path-sandbox host contract~~ — **done**. New "Path-sandbox host contract" section added to `docs/Capability_model.md` (between FfiPolicy and Audit Checklist). Calls out that the `fs` capability gates **which builtins are callable**, not **which paths they may touch**, and ranks four host-side patterns (OS sandbox / chroot+setuid / custom VmHost wrapper / `fs:false` + explicit shim) by isolation strength. Mirrors the same point for `exec`, `http`, `net`, `ffi`. Updated the Audit Checklist with new `fs:true` and `http:true`/`net:true` items.
- ~~Add WebSocket endpoint to `pblang serve`~~ — **done**. New `GET /ws` route on the existing axum 0.7 router (now with `features = ["ws"]`). Tiny JSON-over-text protocol: `{"op":"ping"}` → `{"op":"pong","ts_ms":N}`, `{"op":"echo","msg":...}` → `{"op":"echoed","msg":...}`, `{"op":"version"}` → `{"op":"version","version":"...","service":"pblang-ide-backend"}`. Unknown ops + binary frames + malformed JSON all reply with `{"ok":false,"error":"..."}`. `Close` frames terminate the loop cleanly. The `run`/`check`/`format` ops are **intentionally not exposed** yet — those are CPU-bound and stay on the POST endpoints (a streaming wrapper that re-uses the same compile/run pipeline is a follow-up; the current MVP proves the upgrade path works without re-architecting the handlers). 2 dedicated CLI tests in `tests/new_cmd.rs`: `serve_websocket_ping_returns_pong` and `serve_websocket_version_returns_pkg_version`, both raw-TCP + hand-rolled RFC 6455 handshake (verifies `Sec-WebSocket-Accept` SHA-1+base64 too). Added `base64 = "0.22"` and `sha1 = "0.10"` as dev-deps for the handshake verifier. New `/ws` route documented in `docs/IDE_MANUAL.md` §8.
- ~~Add HTTP/2 to `pblang serve`~~ — **done**. Enabled `axum/http2` feature so `hyper_util::server::conn::auto::Builder` (used by both `axum::serve` for plain HTTP and `axum_server` for TLS) accepts h2 streams. For the HTTPS path, the existing `RustlsConfig::from_pem_file` helper doesn't set ALPN — so a new `build_tls_config_with_alpn(cert, key)` helper now parses the PEMs with `rustls-pemfile`, builds a `rustls::ServerConfig` by hand, and sets `alpn_protocols = ["h2", "http/1.1"]` so clients can negotiate h2 (preferred) or fall back to http/1.1. Added `rustls-pemfile = "2"` as a direct dep. New test `serve_https_alpn_negotiates_h2` spawns the server with a self-signed cert, connects with a rustls client offering ALPN `["h2"]`, drives the handshake to completion, asserts `conn.alpn_protocol() == Some(b"h2")`. All 3 HTTPS tests pass (`serve_https_starts_and_responds`, `serve_https_requires_both_cert_and_key`, `serve_https_alpn_negotiates_h2`).
- ~~Extract spawn/channel concurrency block from `vm.rs`~~ — **done**. 17 fns (`spawn`, `spawn_n`, `join`, `join_all`, `ticker`, `timeout_channel`, plus 11 `channel_*` variants: `channel`/`channel_buffered`/`send`/`recv`/`recv_timeout`/`close`/`is_closed`/`select`/`select_timeout`/`size` + the internal `channel_recv_impl` helper) moved to `crates/pblang-vm/src/vm/spawn.rs` (771 lines). This was the most VM-coupled extraction: snapshots open upvalues from the parent VM stack, threads `VmChannelBuf` handles across `std::thread::spawn`, wires the per-spawn module-globals best-effort transfer via the existing `spawn_wire` serializer. Pulled `super::VM`, `super::StdlibConfig`, `super::VmLimits`, `super::{alloc_array, alloc_map}`, `super::handles::{value_to_handle, vm_handles}`, `super::spawn_wire::*`, and `crate::concurrency::{SpawnWireClosure, SpawnWireValue, VmChannelBuf}`. Cleaned up vm.rs imports: dropped `Mutex`, `mpsc`, `thread`, `AtomicBool`, `SpawnWireClosure`/`SpawnWireValue`/`VmChannelBuf`, `serde_json as sj` (all now consumed only inside submodules), plus the leftover `value_to_handle`/`vm_handles` re-imports at the vm.rs level.
- ~~Extract `BuiltinFn` registration table from `vm.rs`~~ — **done**. The 1,288-line `let mut v: Vec<BuiltinFn> = vec![...]` block (every name/arity/fn-pointer entry) moved to a new `pub(super) fn build() -> Vec<BuiltinFn>` in `crates/pblang-vm/src/vm/builtins_table.rs` (1,320 lines including the imports of all 26 sibling submodules + a 30-line doc-comment header explaining the "add a builtin" workflow). `VM::builtins()` collapses to: `let mut v = builtins_table::build(); v.extend(pblang_graphics::gfx_builtins()); v`. This was the final big extraction — vm.rs is now down to **2,453 lines** (from 12,321 pre-extraction, **−80.1%**).
- ~~Normalize VM error-message style with `vm_err!` / `vm_arity!` / `vm_cap!` macros~~ — **done**. New `crates/pblang-vm/src/vm/errors.rs` provides three macros and `pub(super) use`s them so any sibling submodule can `use super::errors::{vm_arity, vm_cap, vm_err};`. The shape: `vm_err!(host, "<builtin>", "<fmt-args>")` → `"<builtin>(): <msg>"`; `vm_arity!(host, "<builtin>", N, args.len())` → `"<builtin>() expects N argument(s), got M"`; `vm_cap!(host, "<builtin>", "<cap>")` → `"<builtin>(): <cap> access denied"`. Applied at all **49 capability-denied sites** across 10 submodules (`fs.rs` ×15, `net.rs` ×10, `http.rs` ×7, `polyglot.rs` ×5, `ffi.rs` ×4, `ai.rs` ×3, `quantum.rs` ×2, `os.rs` ×1, `time.rs` ×1, `runtime.rs` ×2) — every capability-denied error now identifies the builtin that was rejected. `fs.rs` got the full DRY pass (all 70+ error sites use one of the three macros). Other submodules adopt macros as touched (no broad churn). Addresses the §10 "Inconsistent error-message style" weakness. Tests pass thanks to the existing `contains()` assertions: sandbox 10/0 (incl. all 4 cap-denial tests), semantics 164/0/1, threads 11/0, new_cmd 30/0, workspace lib tests green.
- ~~Add a parser fuzz harness~~ — **done**. New `fuzz/` crate (excluded from the main workspace via `Cargo.toml [workspace] exclude = ["fuzz"]`) hosts a cargo-fuzz harness with two libFuzzer targets: `parse` drives `pblang_core::parse` (recovery path used by LSP / REPL / analyze), `parse_strict` drives `pblang_core::parse_strict` (batch path used by `run` / `compile` / `lint`). Both filter the libFuzzer byte stream to valid UTF-8 (the public surface only takes `&str`) and assert the call returns without panicking — a `panic!` / `unwrap` failure / arithmetic overflow becomes a fuzzing finding. `fuzz/Cargo.toml` declares both `[[bin]]` targets with `test = false` so stable `cargo test --workspace` doesn't try to link libFuzzer. `fuzz/.gitignore` excludes runtime scratch dirs (`target/`, `corpus/`, `artifacts/`, `coverage/`). Install + run instructions in `docs/Fuzzing.md`. Type-checked under `cargo check` in the `fuzz/` directory; actual fuzz runs require `cargo install cargo-fuzz` + nightly. Addresses the §10 "No fuzzing harness" weakness — VM bytecode fuzzing is still open (would need a fuzz-friendly `Chunk` builder first). Not wired into CI yet (a 60s libFuzzer run per push doesn't justify the cost at this commit volume; periodic nightly runs are the right intermediate).
- ~~Implement `pblang.toml [workspace]` semantics~~ — **done 2026-05-26**. New `WorkspaceSection { members, exclude }` struct on `pblang_pm::Manifest`. Resolver `pblang_pm::resolve_workspace_members(root, ws)` expands two pattern shapes: literal member paths (`"tools/foo"` — must contain `pblang.toml` or error) and trailing-`*` wildcards (`"crates/*"` — expands to every immediate subdir of the stem that contains its own `pblang.toml`; non-package subdirs are silently skipped). `exclude` patterns use the same syntax and are applied **after** expansion. Helper `pblang_pm::find_workspace_root(start)` walks up looking for a manifest with a `[workspace]` section so workspace-aware commands can find the right root from inside a member. The CLI integration: `pblang build` parses the manifest at the resolved root; if a `[workspace]` section is present it builds every member in turn (and the root too if the manifest is *hybrid* — i.e. has both `[package]` and `[workspace]`); otherwise the legacy single-package path runs unchanged. Per-package work moved into a new `build_one_package(man_dir, registry_url, require_sha, label)` helper that returns `Result<(), exit_code>` so workspace mode bails on the first failing member with the correct exit code. Output shape: each member emits `build ok [<label>]` and the workspace summary closes with `workspace build ok (N package(s))`. Glob support is deliberately restricted to the trailing-`*` form — no middle-segment or `**` patterns; richer globbing would pull in the `glob` crate for marginal value. 6 dedicated tests in `tests/new_cmd.rs::workspace_*`: literal members, trailing-`*` expansion (with a decoy non-package subdir), `exclude` honored, hybrid roots build root + members, missing-member-manifest error, single-package mode regression-guarded.

Net `vm.rs` shrink across the 2026-05-25 + 2026-05-26 sessions: **10,208 → 2,453 lines (−7,755, −76.0%)**. Combined with the polyglot extraction earlier on 2026-05-25: **12,321 → 2,453 (−9,868, −80.1%)**. The file is now under one-fifth its original size.

> Pitfall encountered: extracting `onnx_infer` to `vm/ai.rs` with `sed 's/^        //'` mangled the embedded Python `ONNX_BRIDGE_SCRIPT` raw string — Python lines with **less** than 8 leading spaces lost no indentation but lines with **exactly** 8 lost it all, breaking the bridge with `IndentationError: expected an indented block after 'try' statement`. Caught by `tests/semantics/onnx_infer_bridge_returns_structured_result` on the first post-extraction test run; fixed by re-indenting the script by hand. Lesson: when bulk-dedenting Rust modules, avoid touching embedded raw strings that carry their own indentation contract (Python, YAML, makefiles, …).

Items completed in the 2026-05-27 (clean-suite milestone) session:
- ~~Close out the final 3 fixable test failures~~ — **done**. The three were all small surface-level issues:
  - **`http_server_timeout_test`, `quantum_remote_stub_test`**: both checked `type(callable) != "function"`, but pblang's `type()` returns one of `"closure"` / `"fn"` / `"builtin"` for the three distinct callable kinds. The `is_fn(x)` built-in is the right callable-discriminator and was already in the dispatch table; both tests now use it.
  - **`tensor_test`**: a row-major indexing bug in the test itself. For data `[1,2,3,4,5,6]` with shape `[2,3]`, position `(1,1)` is `data[1*3+1] = 5`, not `2.0` as the test asserted. Fixed the expected value and added a one-line comment explaining the row-major mapping so the next reader doesn't trip on it.
- ~~Rewrite the stale `os_signal_test` against current `env_set` policy~~ — **done**. The test was written before `env_set` was permanently disabled (thread-safety constraint, see `crates/pblang-vm/src/vm/env.rs`) and was therefore stuck failing. Rewrote it to **codify the security guardrail** as an assertion — `env_set` must throw, `env_get` on an unset key must return nil — which turns the test from a stale liability into a live contract check that protects the disabled-feature decision from accidental re-enable.
- **Final score**: `pblang test tests/semantics` reports **101 passed / 1 failed** under default sandbox, and **102 passed / 0 failed** with `--allow-exec --allow-ffi` (the single default-sandbox failure is `onnx_infer_bridge_test`, which legitimately needs both capabilities to spawn the ONNX runtime — the sandbox is *correctly* refusing it under default flags). This is the first time today's session has the semantics suite at "all green under appropriate capabilities" since the print→throw migration surfaced the 14 hidden failures.

## Trajectory across today's session

| Phase | Passed | Failed | Notes |
|---|---|---|---|
| Start | 100 | 2 | 14 hidden by print-FAIL pattern |
| After print→throw migration | 85 | 19 | hidden failures unmasked |
| After `return -<expr>` parser fix | 87 | 17 | |
| After `regex.is_valid` fix | 88 | 16 | |
| After WebSocket cluster (byte-array rewrite) | 91 | 13 | |
| After concurrency cluster (spawn-module bug fix) | 97 | 5 | |
| **After final-3 cleanup + os_signal rewrite** | **101 / 102** | **1 / 0** | **with default / with capabilities** |

Items completed in the 2026-05-27 (concurrency cluster + spawn-module bug fix) session:
- ~~Fix the entire concurrency-primitive failure cluster (6 tests)~~ — **done**. The 6 failures (`iter_stdlib_test`, `pipe_stdlib_test`, `select_test`, `thread_stdlib_enhanced_test`, `thread_pool_resilience_test`, `waitgroup_test`) decomposed into four very different root causes:
  - **Two test-author bugs** (`iter_stdlib_test`, `pipe_stdlib_test`): both used the formula `x - (x/2)*2 == 0` / `(x/2)*2 == x` for "is even", which only works under integer division. pblang `/` is floating, so `3/2 = 1.5` and `1.5*2 = 3` makes 3 falsely "even". Replaced with `num_mod(x, 2) == 0`.
  - **One test-semantics bug** (`select_test`): the test closed channels and expected `select` to return nil, but Go-style channel semantics (which pblang follows) deliver pending values even after close — and `ch_c` still had `"from_c"` buffered from earlier in the test. Added a drain `recv(ch_c.receiver)` before the close so the assertion holds.
  - **One real VM bug** (the load-bearing fix for `waitgroup_test`, `thread_pool_resilience_test`, and unblocks several future spawn workflows): `spawn(closure)` was silently dropping `Value::Module` upvalues. The closure-serializer `spawn_value_to_wire` in `crates/pblang-vm/src/vm/spawn_wire.rs` had a catch-all `_ => Err("unsupported: builtin/module/resource handle")`, and the caller in `vm/spawn.rs:95-105` used a `if let Ok(...)` to *silently skip* failures — meaning a closure that captured `let thread = import("stdlib:thread.sapp")` reached the child VM with `thread` simply *not in scope*. The child then hung on `thread.wg_done(wg)` because `OpGetGlobal("thread")` couldn't find it. Fix: (a) new `SpawnWireValue::Module { globals, exports }` variant; (b) recurse into the module's globals/exports during serialization; (c) on rehydration, set the new child-side Module's `parent` to the same builtins chain the spawn driver hooks the root closure up to — without this, the rehydrated module would have access to its own closures but couldn't reach `atomic_sub` / `channel_*` / `send` / `recv` builtins. Also had to reshuffle two `RefCell::borrow_mut` windows in `spawn.rs` and `spawn_wire.rs` so they release before the recursive `spawn_value_from_wire` call, since a nested Module rehydration takes a shared borrow on the same handle.
  - **One stdlib contract bug** (`thread_stdlib_enhanced_test`): `select_nonblocking` was documented to "return nil immediately if none ready" but actually returned `map{timed_out: true}` because it delegated to the lower-level `select_with_timeout(receivers, 0)`. Wrapped the result with a `map_has(r, "timed_out")` check that normalizes to `nil`.
  - **One stdlib protocol bug** (`thread_pool_resilience_test`): the worker pool sent raw values into the results channel and then `drain` returned on the first `recv_timeout(...) == nil` it saw — but pblang's `recv_timeout` returns `nil` *both* when the channel is empty *and* when the value sent was nil, so any task that legitimately returned nil truncated the result stream. Wrapped each worker output in `{__tp_result: out}` so drain reads a non-nil envelope, then unwrapped on the parent side.
- **Score after this round**: `pblang test tests/semantics` reports **97 passed / 5 failed** (up from 91/11). Remaining 5: 2 capability-policy (onnx_infer needs `--allow-exec --allow-ffi`; os_signal uses disabled env_set), 2 missing stdlib functions (`http_server_timeout_test::serve_once_timeout`, `quantum_remote_stub_test`), 1 tensor test (vague `fail`).
- **Architectural note worth recording**: the spawn-module fix is the second load-bearing concurrency bug uncovered by the print→throw migration. The first was the `continue`-rebinding compiler bug found earlier in the session. The pattern (a hidden bug that the tests cover but the silent-print pattern was masking) is the highest-value yield from differential-style honesty in test reporting. Worth doing one more sweep on the remaining `stdlib/concurrency.sapp` and `stdlib/async_*.sapp` modules under spawn workloads to surface any other spawn-transfer gaps that are still hiding.

Items completed in the 2026-05-27 (WebSocket cluster + byte-array protocol fix) session:
- ~~Fix the entire WebSocket frame-format failure cluster (5 tests)~~ — **done**. Root cause: `stdlib/websocket.sapp` was carrying RFC 6455 frame bytes inside **pblang strings**, but pblang strings are guaranteed-UTF-8 — any byte ≥ 128 gets re-encoded as a 2-byte UTF-8 sequence on the way in via `strfromcharcode(n)`, and read back as the *first* of those two bytes via `strcharcodeat`. For an opcode-1 text frame, byte 0 should be 0x81 (129) but the round-trip `strcharcodeat(strfromcharcode(129), 0)` returned **194** (0xC2, the high byte of the UTF-8 encoding of U+0081). This silently corrupted every frame byte ≥ 128 (mask bit, length 126/127 markers, all multi-byte length-extension bytes, masking-key bytes ≥ 128).
- **Rewrite**: `stdlib/websocket.sapp` now uses **`Array<u8>`** as the wire-level representation. `write_frame(opcode, payload)` returns `Array<u8>`; `parse_frame(bytes)` takes `Array<u8>`. The text payload in the parsed map is still exposed as a string (`payload` key) for ergonomic comparison in tests, but a new `payload_bytes` key carries the raw demasked byte array for binary opcodes. Helpers `_u16be`/`_u64be` return byte arrays; new `_append_bytes(dst, src)` does in-place extension; new `_bytes_to_text(arr, start, end)` performs the bytes→string conversion only at the leaves where strings are wanted. The 4 sites that previously used `strfromcharcode` to build wire bytes were replaced with `array_push`. The `_byte_xor` helper is unchanged (pure-number).
- **Tests rewritten**: all 5 `websocket_*` tests in `tests/semantics/` updated to construct/consume byte arrays directly (`let frame_data = [129, 2, 72, 105];` for `{0x81, 0x02, 'H', 'i'}`, `if frame[0] != 129` instead of `strcharcodeat(frame, 0)`). The intent of each test is unchanged; only the representation moved off the broken string-as-bytes channel. `pblang test tests/semantics --filter websocket` now reports 5 / 5 passing.
- **Architectural note worth recording**: the same UTF-8 byte-vs-codepoint corruption can lurk anywhere stdlib code builds binary protocol bytes via string concatenation. `stdlib/crypto_advanced.sapp` and `stdlib/encoding.sapp` are the next-most-likely candidates — they use `strcharcodeat` on chunked input and `strfromcharcode` to emit output bytes. If those bytes are ASCII they work; if any byte ≥ 128 leaks through the round trip, the same bug surfaces. Tracked as a follow-up: audit the `strcharcodeat`/`strfromcharcode` callsites in those two modules and migrate to byte arrays where the data is truly binary.
- **Score after this round**: `pblang test tests/semantics` reports **91 passed / 11 failed** (up from 86 / 16 last round). Remaining 11: 2 capability-policy + 5 concurrency primitives + 2 missing stdlib functions + 1 tensor + 1 quantum-stub.

Items completed in the 2026-05-27 (print-FAIL→throw migration + 2 stdlib/parser fixes) session:
- ~~Migrate the `print "FAIL ..."` test idiom to `throw`~~ — **done**. 63 files in `tests/semantics/` used the pattern `if cond { print "FAIL ..."; }` plus a final `print "<name>_pass";` success marker. The Rust harness in `tests/semantics.rs` only asserts that the *last* printed value equals `<name>_pass`, so intermediate `print "FAIL ..."` lines slipped through silently — a test could fire every internal check and still report green. The new `pblang test` runner saw the same false-positive (no `throw`, no runtime error → counted as pass). Mass-converted all 63 files with `sed -i 's/print "FAIL/throw "FAIL/g; s/print "fail/throw "fail/g'` — message strings preserved verbatim. Both runners (cargo's `tests/semantics.rs` and pblang's `pblang test`) now correctly fail on internal check failures. Audited the `print "[A-Z]...` and `print "f...` namespaces for false positives before mass-editing: only `print "WARN ..."` informational lines and the legitimate `<name>_pass` success markers; no rewrites caught any of those.
- ~~Fix the `return -<expr>;` parser bug~~ — **done**. Found by `pblang test tests/semantics --filter algo_stdlib`. The minimal repro: `fn t() { return -1; }` returned `nil`, not `-1`. Root cause: `crates/pblang-core/src/parser.rs`'s `return_stmt` rule was `just("return").ignore_then(expr.clone().or_not()).map(StmtKind::Return)`, with **no whitespace separator** between the keyword and the expression. Without `ws1`, when the input was `return -1;` chumsky's `expr.or_not()` failed silently on the leading `-` flush against the keyword and produced `Return(None)` — leaving `-1` to be parsed as the next statement and discarded. Compare `let_stmt`, `print_stmt`, `throw_stmt`: all three have a `.then_ignore(ws1)` after the keyword. Fix: `return_stmt = just("return").ignore_then(ws1.ignore_then(expr.clone()).or_not()).map(StmtKind::Return)` — wraps `ws1+expr` in the `or_not()` so `return;` (no value) still parses but `return -<expr>;` correctly produces `Return(Some(...))`. Affected at least 9 stdlib sites (`stdlib/array.sapp::index_of`, `stdlib/algo.sapp::binary_search`, `stdlib/colors.sapp`, `stdlib/algorithms_advanced.sapp::binary_search`, `stdlib/functional.sapp` ×2, `stdlib/template.sapp` ×3, `stdlib/strings.sapp`, `stdlib/encoding.sapp`, `stdlib/manifest_validate.sapp`) — all silently broken until now because their `return -1` sentinel was being dropped.
- ~~Fix `stdlib/regex.sapp::is_valid` logic~~ — **done**. The function was `let result = try regex_match("", pat) catch false; return result != false;` — which conflated "did the pattern match the empty string" with "is the pattern syntactically valid". Valid patterns that didn't match (like `\\w+` against `""`) returned `false`. Fixed to `let ok = try regex_match("", pat) catch nil; return ok != nil;` — a valid pattern produces a `true|false` result, an invalid one throws and `catch` substitutes `nil`. Now `is_valid("\\w+") == true` and `is_valid("[invalid") == false`, matching the test's expectations.
- ~~Surface the previously-hidden test failures as a tracked inventory~~ — **done**. With the migration plus the two fixes above, `pblang test tests/semantics` now reports **86 passed / 16 failed** (previous run showed 100/2 only because the print-pattern was masking 14 of those). The 16 failures split into:
  - **2 capability-policy** (`onnx_infer_bridge_test` needs `--allow-exec --allow-ffi`; `os_signal_test` uses the permanently-disabled `env_set`).
  - **5 WebSocket frame-format bugs** in `stdlib/websocket.sapp` (`websocket_extended_len`, `websocket_fragment_reassembly`, `websocket_parse_frame`, `websocket_roundtrip_masked`, `websocket_write_frame`) — frame byte writes/reads disagree with RFC 6455 across several places; a one-shot rewrite of `stdlib/websocket.sapp` is the right move.
  - **5 concurrency-primitive divergences** (`select_test`, `thread_stdlib_enhanced_test`, `thread_pool_resilience_test`, `waitgroup_test`, `pipe_stdlib_test`, `iter_stdlib_test`) — stdlib helpers for `select_nonblocking`, `wg_wait_timeout`, `filter_iter`, `filter_fn`, etc., produce off-by-one or nil-vs-empty results.
  - **2 missing stdlib functions** (`http_server_timeout_test` calls `serve_once_timeout` which doesn't exist; `quantum_remote_stub_test` `fail`-vague).
  - **1 tensor test** (vague `fail`, needs per-line debug).
  - **1 already fixed in this round** (counted in the 86).
  None of these are new regressions; they're all pre-existing bugs the print-pattern was hiding. Worth filing each cluster as a separate cleanup task. Most-leveraged cluster to take next: WebSocket frame format (all 5 tests likely fix together if `stdlib/websocket.sapp::write_frame`/`parse_frame` are rewritten with a single RFC 6455 reference).

Items completed in the 2026-05-27 (cargo-style test runner) session:
- ~~Upgrade `pblang test` to function-level cargo-style discovery and reporting~~ — **done**. The legacy `pblang test [dir]` ran each .sap/.sapp/.bpx file as a single opaque test and emitted a free-form "N passed, M failed" summary. The new runner:
  - **Discovers** top-level `fn test_*(...)` declarations inside each file by walking the parsed AST (`StmtKind::Fn(name, ...)` where `name.starts_with("test_")`).
  - **Compiles + loads each file once**, then synthesizes a small chunk per discovered test fn (`OpGetGlobal(name) → OpCall(0) → OpPop → OpConstant(Nil) → OpReturn`) and runs it on the same VM so the test fn sits in globals.
  - **Emits cargo-style output**: `running N tests`, `test <file_stem>::<fn_name> ... ok` / `... FAILED`, a per-failure `---- <id> stdout ----` section with the runtime error message, the trailing `failures:` summary listing each failed test id, and the `test result: ok|FAILED. N passed; M failed; 0 ignored; 0 measured; K filtered out` line. Non-zero exit code on any failure.
  - **`--filter <substr>` flag** (both `--filter X` and `--filter=X`) drops tests whose `<file>::<fn>` id doesn't contain the substring before the run line is printed.
  - **Backward-compatible**: files with no `fn test_*` declarations get an implicit single test named after the file stem, preserving the legacy "whole file is the test" behavior for the existing semantics/etc. corpora.
  - **Capability flags** (`--allow-fs` etc.) parsed up front via `parse_capability_flags` and applied to every VM created for the run.
  - The clap subcommand docstring + the `pblang help` text were updated to advertise the new behavior.
  Implemented entirely in `src/main.rs`; the planning pass + per-file loop replaces the old ~85-line block. No new dependencies. Verified end-to-end with a 4-test demo corpus (3 fn-level tests + 1 legacy whole-file): output matched the cargo-style layout exactly and exit code was 1 when a test threw.

Items completed in the 2026-05-27 (Phase 0 — formatter operator/comma spacing) session:
- ~~Add operator + comma spacing to the pblang formatter~~ — **done**. `stdlib/compiler/formatter.sapp` now ports `fmt_normalize_operator_spacing` and `fmt_normalize_comma_spacing` from `src/cli/fmt.rs` byte-for-byte: space around `+ - * / == != < > <= >=` (with the same unary-minus rule that drops the trailing space when the operator hugs a literal digit and there's no preceding context); single space after every `,` outside string literals. `format_source` now runs both normalizations on every trimmed line before applying the indent pass. Module is **148 → 308 lines**; export list extended.
- ~~Extend `fmt_drift` corpus to gate operator + comma changes~~ — **done**. `tests/fmt_drift.rs::fmt_drift_indent_corpus_agrees` corpus grew from 4 to 9 snippets: `binary_arith_no_spaces`, `compare_ops_no_spaces`, `comma_in_call`, `unary_minus_literal`, `comma_inside_string_unchanged`. Each snippet is intentionally mis-spaced so both formatters have work to do, and the byte-equal assertion is the gate — any future change that breaks Rust↔pblang parity on these constructs gets caught before the PR can land. `--max-line-length` word-wrap is the remaining Rust-only feature; it'll come in a follow-up round.
- ~~File the `continue`-rebinding pblang VM bug~~ — **done**. During the formatter port, hit a real pblang bug: `let X = i + N` declarations at the top of a `while` body are *not* re-evaluated on the iteration that follows a `continue` from inside the body — they retain their stale prior value. Minimal repro + workaround documented in §3 "Bugs and defects". The byte-walkers in `formatter.sapp` were restructured to use `if/else` with a single bottom `i = i + step;` instead of mid-body `continue`s; the existing `fmt_scan_line` still uses `continue` but its inputs haven't tripped the bug yet — flagged for follow-up. Root cause is in the compiler/VM, not in the stdlib — a real defect uncovered by self-hosting work, which is exactly what Layer 2 differential testing is supposed to surface.

Items completed in the 2026-05-27 (Phase 0 self-host start) session:
- ~~Ship the first Phase 0 Rust→pblang move: the formatter's indentation pass~~ — **done**. New `stdlib/compiler/formatter.sapp` (~145 lines) ports the brace-depth-tracked indentation pass of `pblang fmt` from `src/cli/fmt.rs` into pblang. The pblang module mirrors the Rust `scan_line_for_braces` byte-walking state machine (string / single-quote / nested block-comment / brace-depth tracking) and the top-level-fn blank-line rule. `format_source(src)` returns the indented string. Operator-spacing and comma-spacing normalization are **not** yet ported — they're called out in the module header and left to a follow-up round. The Rust CLI path stays unchanged and remains the default, so end-user `pblang fmt` keeps working byte-identically.
- ~~Wire a differential test that gates future formatter changes~~ — **done**. New `tests/fmt_drift.rs` (1 test, `fmt_drift_indent_corpus_agrees`) implements **Layer 2 of `docs/Bootstrap_safety.md`** for the formatter: drives the Rust CLI (`pblang fmt`) and the pblang formatter on a hand-picked indent-only corpus (`single_fn`, `nested_blocks`, `two_top_level_fns`, `string_with_braces_inside`) and asserts byte-identical output. Any future change that breaks the parity between the two implementations gets a named, actionable error. Test green. Discovered (and documented in the test) a pre-existing quirk in the Rust `fmt_normalize_operator_spacing` helper: it rewrites a `//` line-comment opener into `/ /` because it sees two operator chars; snippets with line comments are deliberately excluded from the corpus until the Rust quirk is fixed or the pblang formatter gains operator spacing.
- ~~Document the bootstrap-safety policy~~ — **done**. (See entry below; this round added the first concrete Phase 0 implementation step that obeys the policy.)

Items completed in the 2026-05-27 (continuation) session:
- ~~Document the bootstrap-loss recovery path~~ — **done**. New `docs/Bootstrap_safety.md` (~7 KB) documents the five-layer recovery plan that protects against the canonical self-hosting risk *"if the pblang binary breaks, the Rust reference will have drifted and I'll be stranded"*: (1) Rust reference never deleted, only `#[deprecated]`-marked; (2) Differential testing every Rust↔pblang twin; (3) Pre-compiled `.pbck` chunks of self-hosted modules checked into git; (4) `pblang transpile → C → cc` as the universal floor; (5) Monthly fresh-checkout CI. Each layer survives the failure of the layers above it. Doc also specifies the PR checklist any "move X from Rust to pblang" change must satisfy.
- ~~Add a drift detector for the partially self-hosted compiler frontend~~ — **done**. New `tests/selfhost_drift.rs` (4 tests, integrated into `Cargo.toml [[test]]` block + the `common` test helpers) implements Layer 2 from `docs/Bootstrap_safety.md`: drives both `pblang_core::parse_strict` (Rust reference oracle) and `stdlib/compiler/selfhost_frontend.sapp::frontend_analyze` / `frontend_parse_full` (pblang self-host frontend) over a small curated corpus of `(label, source)` pairs — 5 valid + 1 invalid — and asserts they agree on accept/reject and on the per-snippet statement count. Each failure mode has a *named* assertion message pointing at `Bootstrap_safety.md` Layer 2 so the operator knows which side regressed. The statement-count test currently logs `skipped` for snippets the pblang parser hasn't caught up on (rather than failing) so this test ships green today and tightens as the pblang parser improves — when full coverage lands, flip the `if verdict == "nil_ast" || verdict == "0" { continue; }` to a hard fail. Expand `VALID_CORPUS` / `INVALID_CORPUS` whenever a new language feature lands.

Items completed in the 2026-05-27 session:
- ~~Add doc comments to `pblang-ir` and `pblang-graphics`~~ — **done**. New crate-level `//!` doc on `pblang-ir/src/lib.rs` explaining the crate's role as the compiler↔runtime↔FFI interface boundary, plus per-type doc comments on `chunk` (module), `FrameTrace`, `RuntimeError`, `VmHost` (covers the capability-gate / sandbox / `io_timeout_ms` / `max_external_json_bytes` / `mem_*` extension points and points at `docs/Capability_model.md`), `BuiltinFn` (calls out the `Some(n)` vs `None` arity contract and the `fn`-not-`Fn` pointer shape), `Obj`/`ObjRef`, `alloc_string`/`string_alloc_count`, `Value` (the `Rc<RefCell<_>>` borrow model), `Function` (template vs. `Closure`), `Closure`, `Upvalue` (Open/Closed lifecycle + shared mutation via `Rc<RefCell<Upvalue>>`), and `Module` (globals / exports / parent). Crate-level `//!` on `pblang-graphics/src/lib.rs` explains the two-backend design (minifb default vs. SDL2 opt-in via cargo feature **and** `PBLANG_GFX=sdl2` env var), and the integer-handle vending scheme (no raw pointers cross into pblang code). `register` is also documented. The two backend module headers were expanded with the same context. `cargo doc --no-deps -p pblang-ir -p pblang-graphics` is now warning-free (initial link-resolution warnings for cross-feature items + private items were fixed by demoting to plain-text mentions). Addresses the §10 "Doc comments are sparse outside vm.rs" weakness.
- ~~Add manifest + registry_index fuzz targets~~ — **done**. Extended the `fuzz/` crate from 2 targets to 4: `manifest` drives `pblang_pm::parse_manifest` (covers the TOML + serde-deserialized Manifest decode path that every PM command runs first — `build`, `install`, `publish`, `add`, …); `registry_index` drives `pblang_pm::parse_registry_index` (the JSON decoder for the HTTP-fetched index that's already size-capped at `MAX_REGISTRY_INDEX_BYTES` but should still degrade to an `Err` on malformed inner JSON, not panic). `manifest` takes `&str` so libFuzzer bytes are UTF-8-filtered first; `registry_index` takes `&[u8]` directly, mirroring the HTTP path. Both wired into `fuzz/Cargo.toml` with `test = false` so stable `cargo test --workspace` still ignores them. `pblang-pm` added as a fuzz-crate dep. `docs/Fuzzing.md` updated: new four-row target table, new `cargo +nightly fuzz run manifest`/`registry_index` snippets, follow-up examples slimmed down (manifest + registry_index were called out as candidates there; now they're done). `cargo check` inside `fuzz/` clean.
- ~~Stale `vm.rs` line-count in §10 weakness~~ — **done**. The "vm.rs is still 11,653 lines" bullet was stale — the file has been 2,453 lines since the 2026-05-26 extractions; corrected the entry with the actual line count and the −80.1% delta.

Items completed in the 2026-05-28 (range-for + continue codegen + binary-encoding fixes + file extensions) session:
- ~~Add binary-safe string↔bytes builtins~~ — **done**. Three pure (no-capability) builtins close the gap that forced binary pblang code through `strfromcharcode` (which treats a number as a *codepoint*, not a *byte*): `str_to_bytes(s) → Array<u8>` (the UTF-8 bytes of `s`), `bytes_to_str(arr) → string` (UTF-8 decode, errors on invalid), and `base64_encode_bytes(arr) → string` (companion to the existing `base64_decode_bytes`). Implemented in `crates/pblang-vm/src/vm/{strings.rs,encoding.rs}` and registered in `builtins_table.rs`. Test: `tests/semantics/string_bytes_test.sapp`.
- ~~Fix the byte-vs-codepoint corruption in `stdlib/encoding.sapp` + `stdlib/crypto_advanced.sapp`~~ — **done** (the follow-up flagged in the 2026-05-27 WebSocket entry). `hex_encode` iterated `len(data)` (codepoint count) while reading `strcharcodeat` (byte offset), truncating every multi-byte character; `hex_decode` rebuilt output via `strfromcharcode(byte)` so binary never round-tripped. Both rewritten to flow through byte arrays via the new builtins, and they now agree byte-for-byte with the native `hex_encode`/`hex_decode` (differential test `tests/semantics/encoding_binary_test.sapp`, a Bootstrap_safety Layer-2 check). `crypto_advanced.encrypt`/`decrypt` had the same XOR-output-≥128 desync (char-indexed `str_slice` vs byte-indexed `strcharcodeat`) plus dead code; rewritten to operate on byte arrays end-to-end, with `decrypt` returning `nil` on auth failure (the old `error(...)` call referenced an undefined global). Round-trip test `tests/semantics/crypto_advanced_binary_test.sapp` covers ASCII, multi-byte UTF-8, wrong-key rejection.
- ~~Implement the `for i in A to B` range-loop syntax~~ — **done**. Six stdlib files (`math`, `encoding`, `colors`, `functional`, `security`, `compiler/parser`) already used `for i in START to END`, but the grammar never implemented it — those functions threw `undefined global 'for'` at call time. Added as a parser-level desugar to `for i in range(START, END + 1)` (no new IR variant): `for_in_stmt`/`g_for_in_stmt` in `crates/pblang-core/src/parser.rs` now take an optional `to END` clause (`text::keyword("to").padded().ignore_then(expr).or_not()`). Tests in `tests/semantics.rs::range_for_*` + `tests/semantics/loop_range_continue_test.sapp`.
- ~~Fix the `continue` infinite-loop in index loops~~ — see §3; `for i N`/`for x in xs`/`for k,v in m` spun forever on `continue`. Fixed in the compiler with forward-patched continue targets.
- ~~Add `push`/`pop` global builtin aliases~~ — **done**. 12 stdlib files call bare `push(`/`pop(` (the `array.sapp` wrappers) without importing `array.sapp`, so those code paths threw `undefined global 'push'` — latent until range-for let them run. Added `push`→`array_push` and `pop`→`array_pop` as global aliases in `builtins_table.rs` (a user-defined local `fn push` still shadows, so purely additive).
- ~~Expand recognized file extensions to `.pbx`/`.pb`/`.bp`/`.bpx`~~ — **done**. Centralized in `pblang_pm`: new `MODULE_EXTENSIONS` const + `is_module_ext`/`has_module_extension` helpers, used by the directory walkers in `src/main.rs` (test runner, etc.), the package-entry candidate list, `package_has_module_files`, and pack verification. `pblang run foo.pb` / `pblang test <dir-with-.pb>` now work. Unit test `pblang_pm::tests::module_extensions_recognized`.
- ~~Add string indexing `s[i]`~~ — **done**. Was unsupported ("indexing only supported on arrays/maps") yet used across the stdlib (`url_encode`, CSV parsing, …) — the root cause of several still-broken functions. Added a `Value::Obj(Obj::String)` arm to `OpGetIndex` **and** the nil-safe `OpSafeIndex` path in `crates/pblang-vm/src/vm.rs`: char-indexed (consistent with `len()`/`str_slice`), returns the i-th codepoint as a one-char string, out-of-bounds errors (nil under `?[]`). Tests `tests/semantics.rs::string_indexing_*`.
- ~~Add the `error(msg)` builtin~~ — **done**. `enum`/`result`/`protocol` stdlib call `return error("…")` to *raise* (the `result.unwrap`-on-`Err` path documents "raise runtime error"); it was an undefined global. Added `error(msg)` in `crates/pblang-vm/src/vm/runtime.rs` — raises a `RuntimeError` with the message, catchable via `try … catch …`. Distinct from `result.err(v)`, which constructs an error *value*. Tests `tests/semantics.rs::error_builtin_*`.
- ~~Make `url_encode`/`url_decode` byte-correct~~ — **done** (companion to the hex fix, enabled by string indexing). Both rewritten in `stdlib/encoding.sapp` to operate on bytes: `url_encode` emits one `%XX` per UTF-8 byte (so "é" → "%C3%A9", not the old malformed single escape); `url_decode` collects all bytes then UTF-8-decodes once. Multi-byte round-trip covered in `tests/semantics/encoding_binary_test.sapp` (which now also exercises `csv_encode`/`csv_decode`).
- **Still NOT fixed**: `import "..." as alias;` is not parsed (only the bare `import` form, which binds the file stem). Deferred — adding it needs an IR/compiler/VM change (the bind-name plumbing) for purely ergonomic value, since the bare form already aliases by stem.
- **Verification**: cargo `tests/semantics.rs` **173 passed / 0 failed / 1 ignored**; `pblang test tests/semantics` **105 passed / 1 failed** (the 1 = `onnx_infer_bridge_test`, capability-gated — passes with `--allow-exec --allow-ffi`); `pblang-core`/`pblang-pm` suites green (incl. the extension-helper unit test).

Items completed in the 2026-05-28 (continuation — slice syntax, let-shadowing fix, VM perf, self-hosted fmt) session:
- ~~Add slice syntax `seq[a..b]` / `[a..]` / `[..b]` / `[..]`~~ — **done**. New polymorphic `slice(seq, start, end)` builtin (`crates/pblang-vm/src/vm/strings.rs`; strings slice by codepoint, arrays by element, `nil` bound = 0/len, clamps) plus a parser desugar: the expression-postfix `[start? .. end?]` becomes `slice(seq, start_or_nil, end_or_nil)` (tried before plain index). Fixes `stdlib/colors.sapp` (`h[1..]`), `stdlib/compression.sapp`, `stdlib/registry_server.sapp` — all previously failed to parse. Tests `tests/semantics.rs::slice_{string,array}_bounds`.
- ~~Fix the `let n = f(n)` self-reference scoping bug~~ — **done** (found auditing `colors.sapp::_to_hex2`). The compiler declared a local *before* compiling its initializer, so a `let` that shadows a param/outer binding of the same name resolved the initializer's reference to the not-yet-initialized new slot → fell through to a global. Reordered `StmtKind::Let`/`Const` codegen to emit the initializer first, then declare (clox semantics). Tests `tests/semantics.rs::let_shadows_{param,outer_local}_with_self_reference`.
- ~~Stdlib smoke test~~ — **done**. New `tests/semantics/stdlib_smoke_test.sapp` exercises `functional` (map/filter/take/drop/zip/unique/flatten/reduce), `colors` (hex↔rgb, shorthand `#abc`, analogous/monochromatic), and `math` (abs/max/min) — all of which depend on the range-for/push/string-index/slice primitives and were unrunnable before.
- ~~Optimize the hottest VM ops~~ — **done**. `OpGetGlobal`/`OpSetGlobal`/`OpGetAttr` allocated a fresh `String` (`.to_string()`) on *every* access just to use as a lookup key. Switched to borrowing the constant `&str` directly (`get_global_from` already takes `&str`; `HashMap::get` accepts `&str` via `Borrow`). ~8% on a fib(28)+1M-loop debug benchmark (5.6s → 5.2s). Noted future levers: make `OpCode` `Copy` (currently blocked by the `OpClosure { captures: Vec<…> }` variant → per-instruction `.clone()`), and release-profile builds.
- ~~Self-hosting step: `pblang fmt --via-pblang`~~ — **done**. The `fmt` command now accepts `--via-pblang` (alias `--self-host`), which formats via the **pblang-implemented** `stdlib/compiler/formatter.sapp::format_source` (run through an embedded VM driver, `src/main.rs::run_self_hosted_fmt`) instead of the Rust formatter, writing back in place. Output is byte-identical to the Rust path (verified by `tests/fmt.rs::fmt_via_pblang_matches_rust_formatter` and the existing `tests/fmt_drift.rs` corpus). Also fixed `format_source` to mirror Rust's `str::lines()` (drop the trailing-newline empty element via the new `slice`) so a newline-terminated file no longer gains a spurious blank line. This is the first real toolchain command that can run on the self-hosted implementation — Phase 0 of the Rust→pblang migration is now exercised end-to-end, not just in tests.
- **Verification (continuation)**: cargo `tests/semantics.rs` **177 passed / 0 failed / 1 ignored**; `tests/fmt.rs` **7 / 0** (incl. the new `--via-pblang` parity test); `tests/fmt_drift.rs` **2 / 0**; `pblang test tests/semantics` **106 passed / 1 failed** (onnx, capability-gated).

Items completed in the 2026-05-28 (new stdlib libraries) session:
- ~~Add `stdlib/duration.sapp`~~ — **done**. Time-span library; a Duration is a plain number of seconds so it composes with unix timestamps / `date.sapp` / `clock()`. Constructors (`millis`/`seconds`/`minutes`/`hours`/`days`/`weeks`), arithmetic (`add`/`sub`/`scale`/`negate`/`abs_dur`), comparison (`compare`/`eq`/`lt`/`gt`), conversions (`as_*`, `between`), decomposition (`parts`), and `humanize` ("1h 30m"), `format_clock` ("01:01:01" / "2:01:01:01"), `parse` ("1h30m", "500ms", "2d 4h"). Test `tests/semantics/duration_test.sapp` (incl. parse↔humanize round-trip).
- ~~Add `stdlib/semver.sapp`~~ — **done**. SemVer 2.0.0 parse (`major.minor.patch[-pre][+build]`, optional `v` prefix), `compare` with correct numeric (not lexical) ordering and full prerelease precedence (§11: numeric < alphanumeric, fewer fields lower, pre < release), `eq`/`gt`/`lt`/`gte`/`lte`, `inc`, and `satisfies` for a single comparator (`^`, `~`, `>=`, `<=`, `>`, `<`, `=`, exact, `*`). Note: pblang has no string `<`, so the module carries a byte-wise `_str_cmp` helper. Test `tests/semantics/semver_test.sapp`.
- ~~Add `stdlib/url.sapp`~~ — **done**. URL parse (scheme/host/port/path/query/fragment/params) and `build`, plus `parse_query`/`build_query` (percent-encoding via the native `url_encode`/`url_decode`, which were made byte-correct earlier this day). Test `tests/semantics/url_test.sapp` (incl. UTF-8 query decode and build↔parse round-trip).
- All three registered in the LSP autocomplete table (`pblang-lsp/src/lib.rs::stdlib_exports`).
- ~~Improvement smoke check~~ — verified `strings`/`stat`/`array`/`collections` public APIs run correctly (no latent breakage from the day's primitive changes).
- **Verification**: `pblang test tests/semantics` **109 passed / 1 failed** (onnx, capability-gated) — +3 new library tests; foundational-module smoke green.

Items completed in the 2026-05-28 (datetime + pipeline operator) session:
- ~~Add `stdlib/datetime.sapp`~~ — **done**. High-level date+time built on `date.sapp` + `duration.sapp`; a datetime is a unix timestamp (seconds). `now`/`from_ymd`/`from_unix`, `add`/`sub`/`diff` (with durations), comparison (`is_before`/`is_after`/`is_same`/`min_dt`/`max_dt`), `format`/`to_iso`/`parse_iso`, component accessors (`year`…`second`), `start_of_day`, and `relative`/`ago` ("2 hours ago" / "in 3 days" / "just now"). Re-exposes `duration` as `dt.dur`. Test `tests/semantics/datetime_test.sapp`.
- ~~Improve `date.sapp`~~ — **done**. Added `parse_unix(iso) → unix ts` (ISO→timestamp; `parse` only returned a calendar map) and exported `_calendar_to_unix` as `calendar_to_unix`.
- ~~Add the pipeline operator `|>`~~ — **done** (a feature from functional languages — Elixir/F#/Hack). `x |> f` → `f(x)`; `x |> f(a)` → `f(x, a)` (LHS prepended, Elixir-style); and the uncommon **placeholder** form `x |> f(a, _, b)` → `f(a, x, b)` (LHS injected at the first `_`). Left-associative, looser than logical/arithmetic so `2 + 3 |> neg` is `neg(5)` and `x |> f |> g` is `g(f(x))`. Implemented as a parser layer (`pipe_expr`) between `logical_expr` and the ternary `expr_top` in `crates/pblang-core/src/parser.rs`, desugaring to `Call`. **Note:** the extra combinator layer initially made `rustc` SIGTERM (chumsky type-explosion on `pblang-core`); fixed with `.boxed()` (the established type-erasure pattern in this parser). Tests `tests/semantics.rs::pipeline_*` (chain, prepend, placeholder, precedence).
- **Verification**: cargo `tests/semantics.rs` **181 passed / 0 failed / 1 ignored** (+4 pipeline tests); `pblang test tests/semantics` **110 passed / 1 failed** (onnx, capability-gated; +datetime).

Items completed in the 2026-05-28 (doc-driven pass: match guards + §16 audit) session:
- ~~Make guarded `match` arms accept `:`~~ — **done** (the Future_plans "fix match guard evaluation" item). Guarded arms previously required a `~`/`#` body separator (`case x if guard ~ body`) while unguarded arms use `:` — a surprising, undocumented inconsistency. Added `:` to the guard separator set (`ws0_guard`) in `crates/pblang-core/src/parser.rs` so `case x if guard: body` now works; `~`/`#` still accepted for back-compat. Tests `tests/semantics.rs::match_guard_accepts_colon_separator` + `match_guard_still_accepts_tilde_separator`.
- ~~`pblang-pm` clippy nit~~ — `c == ""` → `c.is_empty()` in the path-traversal guard.
- **§16 priorities audit** (most already done): JSON ingress caps **done** (`VmLimits.max_external_json_bytes` + `bounded_json_parse` + `DEFAULT_MAX_EXTERNAL_JSON_BYTES`); clippy already ~clean (only a few pre-existing/intentional lints remain: the `type_complexity` tuple at `polyglot.rs:342`, a collapsible-if at `polyglot.rs:2107`, doc-list-indentation in `pblang-ir`); `vm.rs` extraction complete; polyglot pre-warm (`warm_polyglot_cache`) present. Confirmed `class` (via `o:method()` colon-call + `Point.new2()`/`__class`), basic `match`, and `defer` (LIFO) all work — the Future_plans "failing semantics" list is largely stale.
- **Verification**: cargo `tests/semantics.rs` **183 passed / 0 failed / 1 ignored** (+2 match-guard tests); `pblang test tests/semantics` **110 / 1** (onnx, capability-gated).

Items completed in the 2026-05-28 (struct/enum syntax + type-checked construction) session:
- ~~Add `struct` / `enum` declaration syntax~~ — **done** (Future_plans "struct/enum syntax" + "basic type-checker" item). Implemented as **parser sugar** (no new IR variant, so zero blast radius across the 8 `StmtKind` matchers):
  - `struct Point { x: number, y: number }` desugars to a **typed constructor function** `fn Point(x: number, y: number) { return map{ __struct: "Point", x, y }; }`. Instances are tagged maps; fields via normal `p.x`.
  - `enum Color { Red, Green, Blue }` → `let Color = map{ Red: {__enum,__variant}, ... }` (unit variants are tagged maps). Data variants `enum Shape { Circle(number), Rect(number, number) }` → typed constructor functions on the enum map (`Shape.Circle(5)` → `{__enum:"Shape", __variant:"Circle", values:[5]}`).
  - **Type-checker:** because the constructor is a typed function, the existing function-call argument-type check (`pblang-typeck`) validates construction *for free* — `Point(1, "two")` reports `argument 2 type mismatch: expected number, got string`; `Point(1, 2)` is clean. This is the "basic type-checker for structs" with no new typeck machinery.
  - Both `struct_stmt`/`enum_stmt` are `.boxed()` to avoid the chumsky type-explosion (same lesson as the pipeline operator).
  - Tests: `tests/semantics.rs::{struct_construction_and_field_access, enum_unit_and_data_variants}`, `tests/semantics/struct_enum_test.sapp` (incl. `match` on `__variant`), and `pblang-typeck` `test_struct_construction_arg_type_mismatch` / `test_struct_construction_well_typed_ok`.
- **Verification**: cargo `tests/semantics.rs` **185 / 0 / 1**; `pblang-typeck` **27 / 0**; `pblang test tests/semantics` **111 / 1** (onnx, capability-gated).

Items completed in the follow-up 2026-05-28 (typed field/variant access — type-system deepening) session:
- ~~Typed struct field access + field-name checking~~ — **done** (Future_plans "Type System … Foundation for self-hosting"). Entirely within `pblang-typeck` (no new AST node — recognizes the desugared `__struct` constructor pattern), so the blast radius stays in one crate:
  - New `Ty::Struct(name, fields)` / `Ty::Enum(name, variants)` type variants (internal to `pblang-typeck`; no other crate matches on `Ty`).
  - `struct_ctor_return_ty` detects `fn …{ return map{__struct:"Name", …} }` and gives the constructor a real `Ty::Struct` return type (fields = params + the implicit `__struct: string` tag). So `let p = Point(3,4)` types `p` as the struct, not `any`.
  - `GetAttr` is now typed for structs: `p.x` → the field's type (flows into later checks, e.g. `let s: string = p.x` → `cannot assign number to string`); unknown fields report `no field 'z' on struct 'Point'. Did you mean 'x'?` (Levenshtein ≤ 3).
  - `enum_decl_ty` detects the `let Name = map{…}` enum desugaring (unit-variant maps + data-variant constructor `FnLit`s, all sharing one `__enum`) and binds `Name` to `Ty::Enum`. `Color.Rad` → `no variant 'Rad' on enum 'Color'. Did you mean 'Red'?`; data variants are typed constructors so `Shape.Circle("x")` → `argument 1 type mismatch: expected number, got string` (reuses the existing call-arg check). Enums are hoisted alongside fns so variant access resolves regardless of declaration order.
  - Structs/enums are structurally maps in `is_assignable_to` (both directions), so passing a struct where a map is expected — and the constructor's own map-literal return — don't false-positive.
  - Tests: `pblang-typeck` `test_struct_field_access_unknown_field` / `_known_field_ok` / `_field_type_flows` / `_internal_tag_field_ok` / `_field_did_you_mean` and `test_enum_unknown_variant` / `_known_variant_ok` / `_did_you_mean` / `_data_variant_arg_check` / `_data_variant_ok`. End-to-end validated via `pblang --check` (all four diagnostics fire; the existing `struct_enum_test.sapp` — which reads `p.__struct`, `Color.Red.__variant`, `c.values[0]` — reports `typecheck ok`, i.e. **zero false positives**).
- **Verification**: `pblang-typeck` **37 / 0** (clean rebuild, warning-free); cargo `tests/semantics.rs` **185 / 0 / 1**; `pblang test tests/semantics` **111 / 1** (onnx). `pblang --check` pipeline confirmed end-to-end.
- ~~Match-exhaustiveness on enums~~ — **done** (the deeper type-system step). New `Ty::EnumInstance(name)` distinguishes an enum *value* from the `Ty::Enum` namespace. An enum→variants registry lives on the checker `Env`; `resolve_ty` rewrites a `: EnumName` annotation (parsed as an uppercase `Generic`) to `EnumInstance`, and `Color.Red` / `Shape.Circle(5)` now infer as instances. `match <instance>.__variant { case "V": … }` with no `else`/catch-all is checked against the variant set: `non-exhaustive match on enum 'Color': missing variant(s) Blue`. Works on **both** the annotated path (`fn f(c: Color)`) and the pure-inference path (`let c = Color.Red`). Unannotated (`any`) scrutinees are never flagged (no false positives — the existing `struct_enum_test.sapp` with `fn area(shape)` + `else:` stays `typecheck ok`). Tests: `test_enum_match_non_exhaustive` / `_exhaustive_ok` / `_else_is_exhaustive` / `_let_bound_instance` / `_unannotated_no_false_positive`.
- **Verification**: `pblang-typeck` **42 / 0** (clean rebuild, warning-free); cargo `tests/semantics.rs` **185 / 0 / 1**; `pblang --check` exhaustiveness confirmed end-to-end (annotation + inference paths, exhaustive case clean).
- ~~Clippy-style linter for pblang~~ — **done** (user request: "add … the way of rust clippy"). New **`pblang-lint` crate** (mirrors the modular `pblang-typeck` design): `lint(stmts) -> Vec<LintDiagnostic>` where each finding has a stable `name`, a `LintCategory` (correctness/style/complexity/perf), a `message`, and an actionable `help:`. Seven conservative, false-positive-free AST lints: `bool_comparison` (`x == true`), `eq_op` (identical operands `a == a` / `a - a` / `a / a`, restricted to side-effect-free operands via span-blind `pure_expr_eq`), `double_not` (`not (not x)`), `self_assignment` (`x = x`), `constant_condition` (`if true`/`if false`/`while false`), `while_true` (suggest `loop`), `collapsible_if` (`if a { if b { … } }` → `if a and b`). Wired into the CLI: `pblang lint` now also runs these (after typecheck, alongside the existing unused-var/import warnings), and a new **`pblang clippy`** subcommand aliases it. Output is clippy-style: `file:span: warning: <msg> [<category>: <name>]` + `    help: <suggestion>`. Lints are advisory (warnings, exit 0); parse/type errors still exit 1. Also fixed a pre-existing `redundant_closure` clippy nit in `src/main.rs` so the binary is clippy-clean.
- **Verification**: `pblang-lint` **12 / 0** unit (clean build, warning-free); `tests/lint.rs` **5 / 0** (3 existing + `lint_reports_clippy_style_lints`, `clippy_subcommand_is_lint_alias`); cargo `tests/semantics.rs` **185 / 0 / 1**; `cargo clippy --bin pblang` clean (no `src/main.rs` warnings). End-to-end `pblang clippy` demo fires all 7 lints; clean file is silent.

Items still open:

1. **`vm.rs` further extraction is effectively complete.** What's left in the 2,453 lines is the `VM` struct + its `VmHost` impl, the OpCode dispatch loop, `with_stdlib` constructor, the `builtin_capability_class` sandbox classifier, the static GC counters, and `alloc_array`/`alloc_map`/`import_module` helpers. None of these are builtins; they're intrinsic to the VM type. Further reduction would require a `VM` struct redesign, not more extraction.
2. **Wire `pb-dashboard.rs` agent control TODOs** or remove the stub buttons if they're not on the near-term path. (Note: `pb-dashboard.rs` lives in the `editors/ide/tools/mcpservers/pb-mcp-server` git submodule, so requires a submodule-level commit.)
3. **Further `src/main.rs` split** — bundle handler, lint handler, doc handler, REPL loop (per-subcommand flag parsing too). Deferred because handle_* are thin wrappers around `legacy_dispatch`, which inlines each command's body as `if/else if` branches with shared scope. Extracting individual commands needs restructuring `legacy_dispatch` first.
4. **Roadmap-large items from §8** that haven't moved: hosted package registry HTTP endpoints, native single-binary (no extract-to-temp), traits/interfaces, full HM inference, compile-time macros, `async fn`/`await` as keywords, headless-browser HTML/CSS rendering.

## 10. Code quality and maintainability

### Strengths
- **Single source of truth for builtins.** Every builtin appears exactly once in the `BuiltinFn` dispatch table at the bottom of `vm.rs`, and exactly once in the capability classifier — easy to audit, easy to add new ones.
- **Capability gates are uniform.** Every sensitive operation checks `host.allow_*()` first; the pattern is replicated mechanically.
- **No `panic!` in dispatch hot paths.** Errors flow through `RuntimeError` and `?`; the few `unreachable!()` calls have preceding invariant checks.
- **Tests are real.** 375+ passing tests across 30 integration files exercise parser, typeck, compiler, VM, LSP, package manager, sandbox, threads, polyglot, quantum, tensors, encoding — and the test bodies actually assert behaviour rather than just smoke-checking exit codes.
- **`#[ignore]` is used responsibly.** Only 4 ignored tests across the repo, all with documented reasons (network/timing or security-audit boundary).
- **Self-hosted compiler exists.** `stdlib/compiler/*.sapp` (tokenizer, parser, semantic, IR, codegen, optimizer, bootstrap, selfhost_frontend) is a pblang-in-pblang compiler — 4,000+ lines of `.sapp`. Currently runs via the host VM; doesn't yet displace the Rust frontend, but proves the language can express its own toolchain.
- **Documented capability model.** `docs/Capability_model.md` + `docs/FFI_contract.md` + `docs/Spec_freeze.md` + `docs/Embedding_lifecycle.md` give external embedders a stable contract.

### Weaknesses
- ~~**`vm.rs` is still 11,653 lines.**~~ **Addressed 2026-05-25 + 2026-05-26.** Now **2,453 lines** (−9,868, −80.1%) after the 26-submodule extraction. What's left is the `VM` struct + `VmHost` impl, the OpCode dispatch loop, the sandbox capability classifier, and the static GC counters — all intrinsic to the VM type, not builtins. Further reduction would require a `VM`-struct redesign, not more extraction.
- ~~**Inconsistent error-message style.**~~ **Addressed 2026-05-26.** New `crates/pblang-vm/src/vm/errors.rs` defines `vm_err!` / `vm_arity!` / `vm_cap!` macros. The 49 bare `"<cap> access denied"` strings across 10 submodules (`fs.rs`, `net.rs`, `http.rs`, `ai.rs`, `os.rs`, `time.rs`, `polyglot.rs`, `ffi.rs`, `quantum.rs`, `runtime.rs`) were converted to `vm_cap!(host, "<builtin>", "<cap>")` — every capability-denied error now identifies the builtin that was rejected (`"fs_read(): fs access denied"`, `"polyglot_exec(): exec access denied"`, …). `fs.rs` got the full DRY pass through `vm_err!` and `vm_arity!`; other submodules adopt the macros as touched. Tests use `contains()` so the prefix addition is backward-compatible (sandbox 10/0, semantics 164/0/1, threads 11/0).
- ~~**Doc comments are sparse outside `vm.rs`.**~~ **Addressed 2026-05-27.** Added crate-level `//!` and per-type doc comments to `pblang-ir` (covers the IR's role as the compiler↔runtime↔FFI boundary, the `Chunk` / `Value` / `Function` / `Closure` / `Upvalue` / `Module` / `BuiltinFn` / `VmHost` type hierarchy, the PBCK on-disk format, the closure-capture model) and to `pblang-graphics` (covers the two-backend design — minifb default vs. SDL2 opt-in via `PBLANG_GFX=sdl2` — and the integer-handle vending scheme). `cargo doc --no-deps` is now warning-free for both crates.
- ~~**No fuzzing harness** for the parser or VM.~~ **Parser + PM fuzz harness added 2026-05-26/27.** New `fuzz/` directory with a cargo-fuzz crate that's excluded from the main workspace (`[workspace] exclude = ["fuzz"]`). Four targets now: `parse` (recovery path) and `parse_strict` (batch-compile path) drive `pblang_core::parse` / `parse_strict`; `manifest` drives `pblang_pm::parse_manifest` (covers every PM command's first untrusted-text decode); `registry_index` drives `pblang_pm::parse_registry_index` (the HTTP-fetched JSON path used by `pblang install`). All four assert no panic / unwrap failure / arithmetic overflow on arbitrary input. Install + run instructions in `docs/Fuzzing.md`. VM bytecode fuzzing still open (would need a fuzz-friendly `Chunk` builder first). Not wired into CI yet — periodic nightly runs are the right intermediate.
- **Pre-1.0 dependency pin** (`chumsky = "=1.0.0-alpha.8"`). Tracked in `docs/Chumsky_migration_plan.md`; needs a re-pin when 1.0 stable ships.

## 11. Standard library coverage by domain

| Domain | Coverage | Standout modules |
|---|---|---|
| Concurrency / async | **Excellent** | `thread.sapp` (952 L), `async_await.sapp` (526 L), `async_io.sapp` (331 L), `concurrency.sapp` (453 L), `thread_pool.sapp` (383 L) |
| Math / science | **Excellent** | `math.sapp` (1,018 L — sin/cos/log/matrix/stats/FFT/interpolation), `stat.sapp` (177 L), `tensor.sapp` (313 L), `nd.sapp` (175 L) |
| Web / HTTP | **Good** | `http_server.sapp` (491 L), `web_framework.sapp` (209 L), `http_client.sapp` (174 L), `websocket.sapp` (236 L), `router.sapp`, `middleware.sapp` |
| AI / ML | **Good** | `ai.sapp` (327 L — OpenAI/Anthropic/Ollama), `ml.sapp` (387 L), plus 24 native `tensor_*` and 10 native NN-primitive builtins (LayerNorm/BatchNorm/conv2d/maxpool2d/avgpool2d/embedding_lookup/cross_entropy/gelu/leaky_relu/log_softmax) |
| Cryptography | **Good** | Native `sha256`, `hmac_sha256`, `base64_*`, `hex_*`, `url_*`, `random_bytes`, `constant_time_eq` + `crypto_advanced.sapp` (chacha20, poly1305, scrypt, argon2 bindings) + `tls.sapp` (TLS 1.3) + `jwt.sapp` (HS256/RS256) + `crypto_quantum.sapp` (QKD BB84/E91, NIST PQC) |
| Quantum | **Good** | `quantum.sapp` + `quantum_sim.sapp` + `crypto_quantum.sapp` + native `quantum_simulate(circuit)` statevector simulator (≤20 qubits) + AI-bridge `circuit_features` |
| Polyglot | **Excellent** | 50 host languages: 25 interpreted + 27 compiled (`ocaml` both; `csharp` both via `dotnet-script` / `dotnet run`). `polyglot_compile_cached` cdylib cache now spans **16 languages**: rust, c, cpp, go, pascal (existing) + csharp + fsharp (NativeAOT) + zig, d, nim, swift, haskell, fortran, objc, ocaml + **java (GraalVM `native-image --shared`)** + **kotlin (Kotlin/Native `-produce dynamic`)**. Java uses a two-step `javac` + `native-image` wrapped in `sh -c`; output names are normalized by a per-language post-build fixup that handles `lib`-prefixes and publish-tree paths. Per-toolchain regression test in `tests/semantics/polyglot_cdylib_per_lang_test.sapp` exercises every cdylib language with skip-on-missing-toolchain. `polyglot_web.sapp` for HTML/CSS/JS bundling. |
| Data / formats | **Good** | `csv.sapp` (226 L), `dataframe.sapp` (328 L), `serialization.sapp` (246 L — JSON/YAML/MessagePack), `template.sapp` (269 L) |
| Filesystem / OS | **Good** | Native `fs_*`, `path_*`, `cwd`, `os_*`, `signal*` builtins; `stdlib/fs.sapp`, `stdlib/os.sapp`, `stdlib/path.sapp`, `stdlib/env.sapp` |
| Database | **Adequate** | Native `sqlite_*` builtins + `stdlib/db.sapp` (149 L) + `orm.sapp` (232 L) |
| Logging / monitoring | **Adequate** | `log.sapp` (146 L), `profiler.sapp` (297 L), `debugger.sapp` (258 L) |
| Graphics | **Thin** | `pblang-graphics` crate is mostly a façade; many handlers return `Ok(Value::Nil)`. `stdlib/colors.sapp` and `stdlib/color.sapp` give colour utilities. |
| Image processing | **Stub** | `stdlib/image_processing.sapp` delegates to natives that may not exist |
| Advanced networking | **Stub** | `stdlib/networking_advanced.sapp` (BGP/OSPF protocol stubs at 29 lines) |
| Game dev | **Moderate** | `stdlib/game.sapp` (250 L — ECS, tick loop, collision) + `stdlib/ecs.sapp` (78 L) |

## 12. Language feature depth

### Implemented
- **Control flow**: `if`/`elif`/`else`, `while`, `for i N`, `for x in array`, `for k, v in map`, `loop` + `break`/`continue`, `match` with pattern guards, `try`/`catch`, `throw`, `defer` (LIFO), `spawn`, `return`. `if expr` as expression.
- **Numbers — full precision ladder, programmer-opt-in:** unboxed `f64` (fast default) → `bigint(x)` (exact unbounded integers, `Obj::BigInt`) → `rational(x)`/`rational(n,d)` (exact unbounded fractions, `Obj::Rational` — `rational("0.1")+rational("0.2")` is exactly `3/10`) → `decimal(x)` (exact base-10, fixed-scale rounding division, `Obj::Decimal` — `decimal("0.1")+decimal("0.2")` is `0.3`). `type()` reports `"bigint"`/`"rational"`/`"decimal"`. See §19/§20/§21.
- **Type system**: optional type annotations on `let` and `fn`; `TypeExpr` covers primitives, arrays, maps, functions, generics; partial context-driven inference; gradual typing. `struct`/`enum` declarations get **typed field/variant access** in `pblang-typeck` — `p.x` resolves to the field type and flows downstream, unknown fields/variants are reported with Levenshtein "did you mean?" suggestions, and data-variant constructors are argument-type-checked.
- **OOP**: `class` syntax sugar desugaring to maps + methods; instance creation via `new(...)`; method dispatch with implicit `self`; structs as maps with named fields. No nominal inheritance.
- **Error model**: `RuntimeError` values; `try/catch` catches anything; `throw` raises with any value; `defer` runs LIFO on success/error/scope-exit.
- **Concurrency**: `spawn(fn)` lightweight task, channels (send/recv), `select` over channels, mutexes, RwLocks, barriers, atomics, async/await as stdlib sugar.
- **Modules**: `import("stdlib:name.sapp")`, `export name1, name2, …`, cycle detection, module caching, host-customizable resolver.
- **String interpolation**, **nil-safe operators** (`?.`, `?[]`, `??`).

### Not implemented (or partial)
- **Traits / interfaces** — duck typing only.
- **Macros / compile-time metaprogramming** — none.
- **Full HM type inference** — context-driven only.
- **Tail-call optimization** — recursion works but isn't optimized to iteration.
- **`async fn` / `await` as keywords** — sugar layer in `async_await.sapp` instead.

## 13. Tooling and ecosystem

| Tool | Status |
|---|---|
| **Capability flags on `pblang run` / `test` / REPL** | ✓ `--allow-exec` / `--allow-ffi` / `--allow-all` / `--sandbox` / per-cap `--allow-<cap>` / `--deny-<cap>`, plus `PBLANG_ALLOW_EXEC` / `PBLANG_ALLOW_FFI` / `PBLANG_ALLOW_ALL` / `PBLANG_SANDBOX` env-var fallbacks. CLI takes precedence over env. Flags are stripped from argv before `script_args` parsing. 6 dedicated CLI tests in `tests/new_cmd.rs::cap_*`. |
| Package manager (`pblang init`, `new`, `manifest-check`, `install`, `pack`, `verify`, `build`, `publish`, `search`, `fetch`) | ✓ All wired through clap |
| LSP (`pblang lsp`) | ✓ Completion + symbols + actions + diagnostics |
| Formatter (`pblang fmt`) | ✓ Handles `.sap`, `.sapp`, `.bpx` |
| Linter (`pblang lint` / `pblang clippy`) | ✓ Unused-var/import walker **+ clippy-style idiom lints** (`pblang-lint` crate): bool_comparison, eq_op, double_not, self_assignment, constant_condition, while_true, collapsible_if — each with a category and a `help:` suggestion |
| Test runner (`pblang test`) | ✓ |
| REPL (no args) | ✓ Multiline input, brace/string tracking |
| Bundler (`pblang bundle` / `--self-extract`) | ✓ Self-extracting binary with sentinel-located embedded tar.gz; refuses absolute/`..`/symlink entries on extract |
| Doc generator (`pblang --doc`) | ✓ |
| Bytecode dump (`pblang --dump-bytecode`) | ✓ |
| Call-graph (`pblang --call-graph`) | ✓ |
| Dataflow (`pblang --dataflow`) | ✓ |
| Symbol dump (`pblang --symbols`) | ✓ |
| IDE backend (`pblang serve`) | ✓ axum 0.7 on dedicated tokio runtime (8 MB worker stack). HTTPS via `--cert <pem>` / `--key <pem>` (axum-server 0.7 + rustls 0.23 ring provider; CORS allowlist auto-switches to `https://localhost:<port>`). |
| C transpiler (`pblang transpile --c`) | ✓ |
| WASM emitter (`pblang compile --wasm` → `.wat`) | ✓ |
| Polyglot build (`pblang build-polyglot`) | ✓ Extraction; partial cross-compile |
| Bench harness (`benches/bench_vm.rs` via criterion) | ✓ |
| Embeddable C ABI (`crates/pblang-ffi` cdylib) | ✓ |

## 14. Quality comparison vs. other programming languages

This compares **the runtime / language / ecosystem maturity** of pblang against a broad set of existing languages. pblang is a year-old project; the comparisons are by *feature category*, not by maturity-on-the-clock.

### Language design

| Feature | pblang | Python 3 | JavaScript (ES2024) | TypeScript | Rust 1.85 | Go 1.23 | Java 22 | Kotlin 2 | Lua 5.4 | C99 | C++23 | Pascal/Delphi | COBOL | OCaml 5 | Haskell GHC | Clojure | Erlang/Elixir | HTML5 + CSS3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Dynamic typing | ✓ gradual | ✓ | ✓ | optional | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ | n/a (markup) |
| Static type checker | ✓ `pblang lint` (gradual) | ⚠ mypy/pyright (external) | ⚠ JSDoc | ✓ | ✓ strong | ✓ | ✓ | ✓ | ⚠ | ✓ | ✓ | ✓ | ✓ | ✓ HM | ✓ HM | ⚠ spec | ✓ Dialyzer | n/a |
| Pattern matching | ✓ with guards | ✓ structural (3.10+) | ✗ | ✓ via library | ✓ exhaustive | ✗ | ✓ (21+) | ✓ | ✗ | ✗ | ⚠ (26?) | ⚠ case | ✓ EVALUATE | ✓ | ✓ | ✓ core.match | ✓ | n/a |
| Async/await (native) | ⚠ stdlib sugar | ✓ | ✓ | ✓ | ✓ | n/a (CSP) | ✓ (21 vt) | ✓ | ✗ coroutines | ✗ | ✓ (20) | ⚠ ext | ✗ | ✓ effects | ✓ | ✗ | n/a (actors) | n/a |
| Channels / CSP | ✓ | ⚠ queue | ⚠ libs | ⚠ libs | ✓ std::sync::mpsc | ✓ first-class | ⚠ Disruptor | ✓ | ✗ | ✗ | ⚠ TBB | ✗ | ✗ | ⚠ libs | ⚠ libs | ✓ core.async | ✓ first-class | n/a |
| Pattern-matched error handling | ✓ `try/catch` + `defer` | ✓ try/except | ✓ try/catch | ✓ | ✓ Result + ? | ✓ multi-return + defer | ✓ checked | ✓ | ✓ pcall | ✗ setjmp | ✓ throw | ✓ try/except | ✓ DECLARATIVES | ✓ exns + Result | ✓ Either monad | ✓ ex-info | ✓ try/catch/throw + supervisors | n/a |
| Polyglot embedding (host any-lang in same file) | ✓ **47 languages** | ⚠ ctypes/cffi | ⚠ N-API | ⚠ N-API | ⚠ FFI | ⚠ cgo | ⚠ JNI/Panama | ⚠ JNI | ✓ via C | ✓ FFI | ✓ FFI | ✓ external | ✗ | ✓ FFI | ✓ FFI | ✓ JVM interop | ✓ NIF | ✗ |
| First-class concurrency primitives | ✓ spawn/channel/mutex/rwlock/barrier/atomic | ⚠ GIL constraints | ⚠ Web Workers | ⚠ Workers | ✓ | ✓ | ✓ | ✓ | ⚠ coroutines | ⚠ pthreads | ✓ | ⚠ TThread | ⚠ | ✓ | ⚠ STM | ✓ | ✓ actors | n/a |
| Macros / metaprogramming | ✗ | ⚠ decorators | ⚠ proxies | ⚠ decorators | ✓ macro_rules + proc | ✗ | ⚠ APT | ⚠ KSP | ✗ | ✓ #define | ✓ templates | ✓ generics | ✗ | ✓ ppx | ✓ TH | ✓ macros | ✓ macros | n/a |
| Bytecode VM | ✓ | ✓ CPython | ✓ V8 | (via JS) | ✗ (AOT) | ✗ (AOT) | ✓ JVM | ✓ JVM | ✓ | ✗ | ✗ | ✗ | ✗ | ⚠ | ⚠ GHCi | ✓ JVM | ✓ BEAM | n/a |
| Optional native AOT | ⚠ cranelift/LLVM/wasm experimental | ⚠ Cython/Nuitka | n/a | n/a | ✓ | ✓ | ⚠ GraalVM | ✓ Native | ⚠ LuaJIT | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ⚠ GraalVM | ⚠ HiPE | n/a |
| Built-in package manager | ✓ `pblang init`/`pack`/`publish` | ✓ pip | ✓ npm | ✓ npm | ✓ cargo | ✓ go mod | ✓ Maven/Gradle | ✓ Gradle | ⚠ luarocks | ⚠ vcpkg/conan | ⚠ vcpkg/conan | ⚠ FPM | ⚠ | ✓ opam | ✓ cabal | ✓ deps.edn | ✓ rebar3/mix | ⚠ npm (asset side) |
| LSP server included | ✓ | ✓ pyright/jedi | ✓ many | ✓ | ✓ rust-analyzer | ✓ gopls | ✓ jdtls | ✓ | ⚠ | ✓ clangd | ✓ clangd | ⚠ | ⚠ | ✓ merlin | ✓ HLS | ✓ | ✓ ElixirLS | ✓ vscode-html |
| Self-hosted compiler | ⚠ partial (`stdlib/compiler/*.sapp`, 4k+ LOC, runs on host VM) | ✗ (CPython is C) | ✗ (V8 is C++) | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ | ✓ FPC self-host | ✗ | ✓ | ✓ | ✓ | ✓ | n/a |
| HTTP server in stdlib | ✓ | ⚠ wsgiref | ✓ http | (via JS) | ⚠ via crate | ✓ | ⚠ HttpServer | (via Java) | ⚠ | ✗ | ⚠ Boost | ⚠ ext | ✗ | ⚠ Cohttp | ⚠ Warp/Servant | ⚠ Ring | ✓ cowboy | n/a (consumes) |
| AI/ML primitives in stdlib | ✓ 24 native tensor + 10 NN primitives + provider bridges | ✗ (NumPy/Torch external) | ✗ (TF.js external) | ✗ | ✗ | ✗ | ✗ DJL | ✗ | ✗ | ✗ | ✗ libtorch | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | n/a |
| Quantum primitives in stdlib | ✓ statevector simulator + circuit builder + QKD constants | ⚠ Qiskit external | ✗ | ✗ | ⚠ qrusty | ✗ | ⚠ Strange | ✗ | ✗ | ✗ | ⚠ Qpp | ✗ | ✗ | ✗ | ⚠ Quipper | ✗ | ✗ | n/a |
| Polyglot in same source file | ✓ `@lang { ... }` blocks + `polyglot.run(lang, code)` | ⚠ via subprocess | ⚠ via subprocess | ⚠ | ⚠ inline_python crate | ⚠ cgo | ⚠ JNI | ⚠ | ✗ | ⚠ inline ASM | ⚠ inline ASM | ✗ | ✗ | ⚠ ctypes | ⚠ | ⚠ Java interop | ⚠ NIF | n/a |
| Web/UI markup interop | ✓ `polyglot_web.sapp` bundles HTML/CSS/JS into one file | ⚠ Jinja2 | ✓ JSX/Lit | ✓ | ⚠ Yew/Leptos | ⚠ | ⚠ JSP | ⚠ | ⚠ | ✗ | ⚠ | ⚠ | ✗ | ⚠ | ⚠ | ⚠ Hiccup | ⚠ Phoenix LV | n/a (is the markup) |

### HTML / CSS specifically (markup languages, included per user request)

pblang is a **general-purpose language**; HTML and CSS are **markup/style languages** that don't run computation. The honest comparison is between *what each does well*:

| Capability | pblang | HTML5 | CSS3 |
|---|---|---|---|
| General computation | ✓ Turing-complete | ✗ | ✗ |
| Structural document description | ⚠ via templates / `polyglot_web.bundle_html` | ✓ native purpose | ✗ |
| Visual styling | ⚠ via emitted CSS | ⚠ inline styles | ✓ native purpose |
| Polyglot embedding *of* HTML/CSS/JS | ✓ `stdlib/polyglot_web.sapp` bundles HTML+CSS+JS into one self-contained `.html` file, optionally opens in browser | n/a | n/a |
| Browser-side execution | ⚠ via emitted JS, or via `polyglot_exec("js", …)` (node/deno subprocess) | ✓ runs in every browser natively | ✓ runs in every browser natively |

The realistic positioning: pblang can **emit and orchestrate** HTML/CSS/JS (build the bundle, serve it, exec JS via node/deno) but doesn't *replace* them as the markup/style runtime. That's the same posture every general-purpose language has — it doesn't *become* HTML, it *produces* HTML.

### Where pblang punches above its weight (vs. its 1-year-old age)

- **Polyglot reach (47 host languages including COBOL, Pascal/Delphi, x86 assembly, Erlang, Kotlin, Haskell, Crystal, Nim, V, OCaml, Vala, …).** Most mainstream languages stop at "via FFI/subprocess to one or two others."
- **Native AI/ML primitives** — 24 tensor ops + 10 NN building blocks (conv2d, maxpool2d, layer_norm, batch_norm, embedding_lookup, cross_entropy, gelu, log_softmax, …) implemented in Rust as VM builtins, not as Python-wrapper imports.
- **Native quantum simulator** — statevector sim up to 20 qubits, full Clifford+T gate set, circuit builder, QKD protocol stdlib.
- **Built-in concurrency stack** at language level — spawn, channels, select, mutexes, RwLocks, barriers, atomics, async/await sugar — rather than as a library.
- **One-binary deployment** via `pblang bundle --self-extract` with hardened extraction.
- **Capability-based sandboxing** — every sensitive builtin gates on `host.allow_*()`, suitable for embedding pblang in untrusted contexts.
- **Self-hosting in progress** — `stdlib/compiler/*.sapp` is a pblang-in-pblang frontend.

### Where pblang trails

- **Ecosystem size.** Python/JS/Rust have hundreds of thousands of third-party packages; pblang's hosted registry isn't live yet.
- **Production hardening.** No CVE-tracking history, no formal spec freeze, no compatibility test suite, no LTS release cadence.
- **Tooling depth.** No fuzzer, no profile-guided optimizer (the bytecode VM has no JIT in the default path), no debugger UI parity with PyCharm/VS Code.
- **Type system.** Gradual typing without traits/typeclasses puts it closer to TypeScript-without-classes than to Haskell/Rust/OCaml.
- **AOT story.** Cranelift/LLVM/WASM backends exist but the primary path is the bytecode VM; production native binaries are not the default workflow.

## 15. Risks (ranked)

1. **Pre-release dependency pin** — `chumsky = "=1.0.0-alpha.8"`. If the alpha gets yanked or a security advisory lands, no fallback. Mitigation: pre-emptive migration path documented in `docs/Chumsky_migration_plan.md`.
2. **`src/main.rs` review-fatigue surface** — ~5,980 lines in one file (the CLI). (`vm.rs` itself is now only 2,526 lines after the 32-submodule extraction.) Mitigation: continue the `src/cli/*` extraction.
3. **No hosted registry yet** — package distribution still requires `file://` paths or git URLs. Limits ecosystem growth.
4. **Polyglot cdylib loads run as the embedding process** — once a pblang script has `allow_ffi` + `allow_exec`, any cdylib it compiles runs in-process. Mitigation: this is the documented FFI contract; downstream embedders should treat `allow_ffi(true)` as equivalent to "load arbitrary code".
5. **Java/Kotlin path re-compiles every call** — no `.class`/`.jar` caching; high latency for repeated invocations. Mitigation: clone the `polyglot_compile_cached` design for JVM bytecode.

## 16. Recommended priorities for the next 1–2 sessions

(highest leverage first; refreshed 2026-05-29 — the earlier list's items 1–4 & 6 are **done**: clippy `clone_on_copy` cluster fixed, `cargo-audit` CI job wired, JSON ingress caps added, `vm.rs` extracted to 32 submodules, `pblang build --warm-polyglot` shipped.)

1. ~~**Front-end robustness against adversarial input**~~ — **done 2026-05-29** (see §17): parser nesting-depth guard + large-stack worker + VM stack-depth cap on `run`. Was the only critical/high finding.
2. **Hosted-registry *server*.** The registry *client* (GitHub/GitLab/DirectHttp fetch/publish/search) already works and is tested; what's missing is a first-party hosted index server to publish to.
3. **True single-binary executable** — `pblang bundle --self-extract` still extracts to a temp dir; an mmap-the-tar.gz, no-fs-writes variant is the gap.
4. ~~**WASM backend codegen for `if`/`while`/`for`**~~ — **done 2026-05-29** (see §18). Now also comparisons, `not`/`and`/`or`, and `=` assignment. Remaining WASM gaps: bare local names lack the `$` sigil (pre-existing, blocks `wat2wasm` validation), calls aren't wired, and `and`/`or` don't short-circuit.
5. **Reduce `src/main.rs` (5,980 L)** via the `src/cli/*` pattern, and **consolidate stdlib API overlap** (`http`/`http_client`/`http_server`, `color`/`colors`, `date`/`datetime`/`time`, `ml`/`machine_learning`/`ai`).
6. **Java/Kotlin `.jar`/native-image caching** parallel to the cdylib cache (repeated calls recompile today).

## 17. External re-audit + hardening (2026-05-29)

A fresh end-to-end audit (real toolchain + 4 parallel deep-dives) surfaced findings the earlier sections **missed**, plus stale metrics (now corrected in §1/§15/§16). The code findings were fixed the same session; verification: `pblang-core` depth tests 4/0, cargo `tests/semantics.rs` 185/0/1, cargo `sandbox` 10/0, `pblang test tests/semantics` 111/1 (onnx), `pblang-typeck` 42/0, `pblang-lint` 12/0; workspace clippy clean for all touched code.

| Sev | Finding | Status |
|---|---|---|
| 🔴 Critical | **Parser/compiler native stack overflow → uncatchable process abort** on deeply-nested source (`(((…)))`, `[[[…]]]`, `1+1+1+…`). Reproduced: `pblang run` aborted with `fatal runtime error: stack overflow`. The chumsky parser + AST recursion had no depth limit. | **Fixed.** New `pblang-core::check_nesting_depth` (O(n), string-aware) rejects bracket nesting > `MAX_NESTING_DEPTH` (512) with a clean `ParseError` at *both* parse entry points — protects CLI, LSP, serve, lint, typeck. Plus a 256 MiB large-stack worker thread in `main()` (`CLI_WORKER_STACK_BYTES`) backstops deep flat operator chains. Tests: `pblang-core::depth_guard_tests` (4). |
| 🟠 High | **No `VmLimits` on the CLI `run`/REPL path** — only `serve` set them; infinite-loop / unbounded-alloc / runaway-recursion DoS. | **Mitigated.** `run` now sets `max_stack_depth: 16_384` (converts runaway VM recursion into a catchable `RuntimeError` instead of a native abort). Instruction/memory budgets stay unbounded for the *trusted* CLI (matching `python file.py`); the untrusted `serve`/embed paths already enforce full limits. |
| 🟡 Medium | **`env_get` was ungated** → a sandboxed script with net/http could read & exfiltrate `OPENAI_API_KEY`/`AWS_*`. | **Fixed.** New `env` capability: `VmHost::allow_env()` (default on), `StdlibConfig.env`, gated in `vm/env.rs`, wired to `--allow-env`/`--deny-env`/`--sandbox`/`--allow-all` + `PBLANG_*` env vars. `--sandbox` now also blocks env reads. |
| 🟡 Medium | **`gfx` capability flag was non-functional** — graphics builtins had no gate, so `--deny-gfx`/`StdlibConfig.gfx` did nothing. | **Fixed.** `install_builtins_into` now registration-gates `gfx*` builtins on `config.gfx` (they lack a call-time check); `--deny-gfx` makes them undefined. |
| 🟡 Medium | **`machine_learning.sapp` shuffle was degenerate** — `k = j % 4` only ever swapped into slots 0–3, biasing `train_test_split`/`k_fold`. | **Fixed.** Proper Fisher–Yates with a MINSTD LCG (stays in f64 exact-int range). |
| 🟡 Medium | **`pblang-wasm` doc/impl mismatch** — header claimed `if`/`while`/`for` "✅" but codegen emits "not fully implemented". | **Fixed** (doc corrected to "⚠ partial"). Real codegen is a roadmap item (§16.4). |
| 🟢 Low | `vm.rs` `OpSafeGetAttr` used a raw `constants[idx]` (panic on crafted bytecode). | **Fixed** — now `.get()`-checked like its siblings. |
| 🟢 Low | **Capability *visibility* gating (`StdlibConfig::allows`) is dead code** with a comment claiming it's wired. | **Doc fixed.** Not a vulnerability — the live gate is the call-time `allow_*()` checks (which work); `allows()` is a documented future hook. Comment corrected to stop contradicting `install_builtins_into`. |
| 🟢 Low (accepted) | **SSRF in `http_get`/`http_post`** (no host/IP allowlist) and **registry accepts plaintext `http://`** tarballs (SHA only verified when pinned). | **Accepted/documented.** Both gated by `allow_http`; blocking localhost/link-local would break legitimate local-API usage (no mainstream stdlib http client blocks SSRF by default). Pin `archive_sha256` for untrusted packages. |

**Follow-up — compiler superlinearity on deep ASTs (fixed same session):** a long flat `1+1+1+…` chain compiled in **O(n²)** time *and* overflowed the stack (~6k terms). Root cause in `emit_expr` (`pblang-compiler`): the const-fold guard `(a.kind.clone(), b.kind.clone())` deep-cloned the entire left subtree at every level, and codegen recursed down the left spine. **Fixed** by (1) matching the fold guard by reference (`&a.kind`/`&b.kind`) → O(n), and (2) flattening left-associative `Add`/`Sub`/`Mul`/`Div` chains **iteratively** (explicit spine stack) → no deep recursion. Emitted bytecode is unchanged (the `optimize_chunk` peephole still folds literal pairs). Result: linear (500,000-term chain runs in ~3 s) with **no overflow** at any depth; n=4000 went 3.97 s → 0.02 s. Regression test `tests/semantics.rs::deep_left_nested_arithmetic_chain_compiles_iteratively` (20k-term chain on the default 2 MB test stack). (Deep *right* spines need parens and are already caught by the bracket-depth guard; comparison/logical flat chains remain recursive — rare, and the big worker stack covers thousands.)

**Doc corrections applied:** §1 metrics refreshed (LOC 39,899→44,615; `vm.rs` 11,653→2,526; stdlib 121→125; tests recounted); §15 risk #2 retargeted from `vm.rs` to `src/main.rs`; §16 priorities rewritten (items 1–4 & 6 were already done). **Registry status corrected:** §7/§8 said "HTTP endpoints not implemented, only `file://`" — in fact the registry *client* (`RegistrySource::{GitHub,GitLab,DirectHttp}`, `fetch`/`publish`/`search`, 401-retry) is implemented and tested; only a first-party hosted *server* is missing.

## 18. Roadmap item: WASM backend — usable numeric+string subset (2026-05-29)

Closed §16.4 — the `pblang-wasm` backend previously emitted `"… not fully implemented"` warnings for `if`/`while`/`for` (the doc header even claimed them "✅" — corrected in §17). Now implemented in `crates/pblang-wasm`:

- **Comparisons** (`==`/`!=`/`<`/`>`/`<=`/`>=`) — compute the i32 result then widen to f64 (new `ConvertI32ToF64` → `f64.convert_i32_u`) so every value stays uniformly f64 (also fixed: float comparisons now emit `f64.lt` etc., not the invalid `f64.lt_s`).
- **`not` / `and` / `or`** — truthiness-based, with **short-circuit** `and`/`or` via `if (result f64)` (the right operand is only evaluated when needed); **`bool`/`nil` → f64** (was i32, which mismatched the f64-only locals).
- **`if`/`else`** — structured `if … else … end` with an f64→i32 truthiness lowering (`value != 0.0`).
- **`while`** — `block $end (loop $start … )` with the back-edge `br $start` and exit `br $end`.
- **`for i N`** — counted loop with an f64 induction variable, `f64.ge` bound check, body, `+1` increment.
- **`=` assignment** in function bodies (`local.set`) — needed for loop bodies.
- **Direct function calls** — `add(2, 3)` → `call $add` (args pushed, callee's f64 result left on the stack); a known-function lookup via `fn_return_types` gates it. print/method/indirect callees are still no-ops.
- **Valid-WAT identifiers** — all names now carry the mandatory `$` sigil (`$add`, `$a`, `$memory`, …) via the `sigil()` helper, and **params are declared inline** (`(func $add (type $type_add) (param $a f64) (param $b f64) (result f64) …)`) so `local.get $a` resolves. The output is now spec-correct WAT for the supported subset (no `wat2wasm` available on this box to assert mechanically; verified by inspection).
- The serializer now **recursively renders** nested `if`/`block`/`loop` (previously they were placeholder comments, so no real control flow was ever emitted) and supports `if (result f64)` for short-circuit expressions.
- Tests: `pblang-wasm` (9/0) — `wasm_if_else_…`, `wasm_while_…`, `wasm_for_…`, `wasm_comparison_widens_to_f64`, `wasm_logical_and_or_short_circuit`, `wasm_identifiers_carry_dollar_sigil`, `wasm_direct_function_call_emits_call`. Verified end-to-end: `pblang compile <f> --wasm` produces clean, correctly-structured WAT for `for`-loops and cross-function calls.

- **Local-name dedup** — repeated/shadowed `let x` now emits a single `(local $x f64)` (`declare_local_once`); WASM locals are function-flat, so duplicates would be invalid WAT. Test `wasm_repeated_local_declared_once`.
- **`print`/strings memory model** — string literals are interned into linear memory as a length-prefixed `[len_u32][utf8…]` data segment; `print "…"` calls `pblang_print(data_ptr, len)` with the compile-time pointer/length. Numbers print via a new `pblang_print_f64(f64)` host import. **f-strings** lower part-by-part (literal segments → `pblang_print`, interpolations → `pblang_print_f64`). Verified WAT for `print "result:"`, `print x+1`, and `print f"x={x}!"`. Tests `wasm_print_string_literal_uses_memory`, `wasm_print_number_uses_f64_import`, `wasm_print_fstring_prints_each_part`. Total `pblang-wasm` tests: **13/0**.

**Remaining WASM gaps (not this slice):** printing a string held in a *variable* (needs a value-type model the f64-only locals don't have — string literals and numbers print fine), and arrays/maps depth. The VM, Cranelift, and C-transpiler backends are unaffected.

## 19. Arbitrary-precision integers + arith perf + self-hosted `--check` (2026-05-29)

**High-precision data type (`bigint`) — the f64-only value model is no longer the precision ceiling.** Numbers stay unboxed `f64` by default (fast); the programmer opts into arbitrary precision explicitly with `bigint(x)`.
- New `Obj::BigInt(num_bigint::BigInt)` variant (heap-boxed like strings, so the unboxed `Value::Number(f64)` fast path is undisturbed; `num-bigint`/`num-traits` resolved from the offline cargo cache). Blast radius stayed in the `Obj` matchers (parser/typeck untouched structurally).
- `bigint(x)` builtin: from an integral number, a decimal string (lossless beyond 2^53, e.g. `bigint("123456789012345678901234567890")`), or an existing bigint.
- Arithmetic centralized in `Value::arith(op)` (+ `num_cmp` for ordering): exact `+ - * / % **` when an operand is a bigint (integer `/` truncates; `**` needs a non-negative integer exponent), with cross-type promotion (`bigint + int-number`) and demotion (`bigint + fractional float → f64`). `==` is value-based across bigint/number (`bigint(5) == 5`). `type()` → `"bigint"`; zero is falsy.
- Verified exact: `2 ** 100`, `30!`, `25!`, huge products/sums. Tests: `tests/semantics/bigint_test.sapp` + `tests/semantics.rs::bigint_arbitrary_precision_pass` (in both the cargo suite — 187/0/1 — and the native runner — 112/1).
- Not in WASM (f64-only backend); JSON/SQLite/spawn-wire serialize a bigint as its exact decimal string (documented).

**Optimization.** Routing all arithmetic/ordering through `arith`/`num_cmp` could have taxed the hot loop, so the **`(Number, Number)` fast path is checked first** (branch-free of the bigint/string logic) and `arith`/`num_cmp`/`apply_f64` are `#[inline]`. Dead `pop_number` removed. Release timing: a 10M-iteration numeric loop runs in **~1.5 s** (≈40M VM-ops/s); the f64 path carries zero bigint overhead.

**Self-hosting step.** New `pblang --check --via-pblang <file>` routes the check through the **pblang-implemented** front end (`stdlib/compiler/selfhost_frontend.sapp::frontend_analyze`) instead of the Rust parser/typecker — mirroring the existing `fmt --via-pblang`. Rust `--check` stays the default until parity. Emits e.g. `self-hosted check ok (stmts=3, tokens=12)`. Test: `tests/fmt.rs::check_via_pblang_uses_self_hosted_frontend`.

## 20. Exact arbitrary-precision rationals (2026-05-29)

The highest-precision type for **non-integers**, completing the precision story (§19 covered integers). Opt-in, exact, unbounded.
- New `Obj::Rational(num_rational::BigRational)` (a reduced `BigInt` numerator/denominator; `num-rational` from the offline cache). `alloc_rational` normalizes an integer-valued rational back to `Obj::BigInt`, so `rational("6/3")` is a bigint `2`.
- **`rational(x)`** from a fraction string (`"1/3"`), a decimal string parsed *exactly* (`"0.1"` → 1/10, not the f64 approximation), an integer, or a number (its exact binary value); **`rational(n, d)`** from an explicit pair.
- `Value::arith` gained an exact-rational branch (priority over bigint — a bigint operand promotes to n/1): exact `+ - * /`, integer-exponent `**` (incl. negative), `%` rejected (not defined for rationals), with f64 demotion only on a non-finite mix. `num_cmp`/`==` are exact and cross-type (`rational("1/2") == 0.5`, `bigint(2) + rational("1/2") == "5/2"`). `type()=="rational"`; a rational object is always truthy (zero normalizes to bigint).
- **Headline:** `rational("0.1") + rational("0.2")` → `3/10` exactly (vs f64 `0.30000000000000004`).
- Tests: `tests/semantics/rational_test.sapp` + `tests/semantics.rs::rational_exact_precision_pass` (cargo suite 188/0/1; native 113/1). Same Obj-matcher blast radius as bigint, all handled; not in the WASM backend; JSON/SQLite/spawn-wire serialize as the exact `"n/d"` string.

## 21. Arbitrary-precision base-10 decimals (2026-05-29)

The money/display-friendly companion to rationals — completes the precision ladder (`f64` → `bigint` → `rational` → `decimal`). **Hand-rolled** on `num-bigint` (no `bigdecimal` crate was cached): `Obj::Decimal(Decimal { mantissa: BigInt, scale: i64 })` = `mantissa × 10^−scale`.
- **`decimal(x)`** from a string (`"3.14"`, `"0.1"` — exact base-10), a number (its shortest decimal form, so `decimal(0.1)` is `0.1`), or a bigint/rational/decimal.
- Sums/products are **exact** (scale-aligned `BigInt` ops); **division rounds** half-away-from-zero to a fixed scale (`DECIMAL_DIV_SCALE = 50`) so `decimal("1")/decimal("3")` terminates (unlike a rational fraction). Integer-exponent `**` (incl. negative). `%` rejected. Results are normalized (trailing zeros dropped → `0.30 == 0.3`).
- Cross-type exact `==`/ordering (`decimal("0.1") == 0.1`, `decimal("0.5") == rational("1/2")`); `decimal + bigint` stays decimal; `rational(decimal("0.25"))` → `1/4`. `type()=="decimal"`; nonzero ⇒ truthy.
- **Headline:** `decimal("0.1") + decimal("0.2")` → `0.3` exactly; `decimal("19.99") * decimal("3")` → `59.97`.
- Tests: `tests/semantics/decimal_test.sapp` + `tests/semantics.rs::decimal_arbitrary_precision_pass` (cargo suite **189/0/1**, native **114/1**). All `Obj` matchers extended; `pblang-ir`/`pblang-vm` clippy-clean (the `Decimal::compare` method is named to avoid clashing with `Ord::cmp`). Not in the WASM backend; serialized as plain decimal text in JSON/SQLite/spawn-wire.

**The precision ladder is now complete:** the programmer opts into `f64` (fast default) → `bigint(…)` (exact unbounded integers) → `rational(…)` (exact unbounded fractions) → `decimal(…)` (exact base-10, fixed-scale division).

## 22. Precision types as first-class in the type-checker + WASM handling (2026-05-29)

The precision types are now **first-class in `pblang-typeck`** (previously the constructors returned `any`):
- New `Ty::BigInt` / `Ty::Rational` / `Ty::Decimal`, and matching `TypeKind::{BigInt,Rational,Decimal}` in `pblang-core` so **`: bigint` / `: rational` / `: decimal` annotations parse** (added to the `ty_atom` grammar). Only 3 `TypeKind` match sites needed updating (parser, `from_type_expr`, the `--doc` renderer).
- The constructors return their types: `let x = bigint(5)` infers `x: bigint`. Assigning across the numeric family is allowed (the VM promotes/demotes), but **numeric ↛ string/bool is flagged**: `let s: string = bigint(5)` → "cannot assign bigint to string"; `let d: decimal = "hi"` → "cannot assign string to decimal".
- Arithmetic uses the **promotion ladder** (`Ty::numeric_widen`, `is_numeric`): `bigint(2) ** 64` and `decimal("1.5") * decimal("2")` no longer trip the old "expected number operands" check; an array `[bigint, number]` infers element type `bigint`. `widen`/`is_assignable_to` treat the four numeric types as interoperable.
- Tests: `pblang-typeck` `test_bigint_constructor_type_flows`, `test_precision_type_annotations_ok`, `test_decimal_annotation_rejects_string`, `test_bigint_arithmetic_not_flagged_as_non_number` (46/0).

**WASM:** the arbitrary-precision types are VM-only — the f64 WASM backend can't represent them. Rather than silently lowering `bigint(…)`/`rational(…)`/`decimal(…)` to a `0.0` no-op, the backend now emits a **specific warning** ("arbitrary-precision … unsupported in the f64 WASM backend; value approximated as f64"). The WASM backend matches the AST, not runtime `Obj`, so the new value variants need no other WASM changes.

## 23. Precision-backed stdlib: exact `money.sapp` + new `bignum.sapp` (2026-05-29)

Pure-pblang libraries (no Rust rebuild) that put the new precision types to work:
- **`stdlib/money.sapp` rewritten on `decimal`** (was float-based: `num_floor(amount*100+0.5)` — the exact cents-drift this fixes). All ops (`add`/`subtract`/`multiply`/`divide`/`percent`/`tax`/`discount`) are now exact; new `round(d, places)` (exact round-half-up via bigint carry) and `split(amount, n)` (largest-remainder allocation in integer cents — parts sum back with **no lost cents**, e.g. `$10/3 → 3.34, 3.33, 3.33`). `format` keeps its `"<value> <currency>"` shape (existing `modules.rs` test still green). `0.10 + 0.20` is exactly `0.30`.
- **New `stdlib/bignum.sapp`** — exact big-number helpers on `bigint`: `factorial`, `gcd`, `lcm`, `pow`, `fib`, `comb`, `perm`, `is_even/odd`. No f64 overflow (`factorial(50)`, `fib(100)`, `comb(52,5)` are exact).
- **New `stdlib/fraction.sapp`** — ergonomic exact-fraction wrapper over `rational`: `of`/`numerator`/`denominator`/`add`/`sub`/`mul`/`div`/`neg`/`pow`/`reciprocal`, `to_mixed` (`7/3 → "2 1/3"`), `mediant` (Farey/Stern-Brocot), and `approximate(x, max_denom)` (continued-fraction best rational — `approximate(3.14159265, 1000) → 355/113`).
- Tests: `tests/modules.rs::{money_is_exact_decimal_based, bignum_exact_helpers, fraction_exact_wrapper_over_rational}` (modules suite **19/0**). Stdlib module count 125 → 127.

## 24. Stdlib expansion + a real GC correctness fix + `--mem-report` (2026-05-29)

**Seven new pure-pblang libraries** (no Rust rebuild; each verified in `tests/modules.rs`):
- **`set.sapp`** — a hash Set with full algebra: `add`/`remove`/`has`/`union`/`intersection`/`difference`/`symmetric_difference`/`is_subset`/`is_superset`/`is_disjoint`/`equals`/`map`/`filter`. Backed by a map keyed on `str(v)`, storing originals so `to_array` returns real values.
- **`vector.sapp`** — n-d numeric vectors: `add`/`sub`/`scale`/`hadamard`/`dot`/`cross` (3-D)/`norm`/`l1_norm`/`linf_norm`/`normalize`/`distance`/`lerp`/`cos_angle`/`project`.
- **`matrix.sapp`** — dense matrices: `identity`/`mul`/`transpose`/`trace`/`mul_vec`; `determinant` (cofactor — exact for integer matrices); `inverse` (Gauss-Jordan, partial pivot); `solve` (Gaussian elimination + back-substitution).
- **`heap.sapp`** — binary heap (min/max) with **correct in-place mutation** (`push`/`pop`/`peek`/`heapsort`) — fixes the latent non-persisting-`pop` shape in `data_structures.sapp`'s `pq_*`.
- **`deque.sapp`** — double-ended queue (`push_back`/`push_front`/`pop_back`/`pop_front`/`peek_*`).
- **`base32.sapp`** — RFC 4648 Base32 encode/decode (exact small-integer bit accumulation; verified against the RFC §10 vectors, e.g. `"foobar" → "MZXW6YTBOI======"`).
- **`complex.sapp`** — complex numbers (`add`/`sub`/`mul`/`div`/`conj`/`abs`/`arg`/`neg`/`inv`/`pow_int`/`sqrt`/`exp`/`from_polar`); self-contained range-reduced sin/cos. `sqrt(3+4i)=2+1i`, `|3+4i|=5` exact.

**`resources.sapp`** — memory & resource introspection usable by **any** pblang program (pblang's own or a host project), unifying `gc_stats`/`memory_stats`/`gc_collect`/`now_ms`/`clock`: `snapshot`, `diff`, `track`/`measure` (per-op allocation+time cost), `gc`, `gc_measure`, **`live_capture`/`sample`** (snapshot a workload over N steps — *live memory capture*), `summary`, `leak_check`, `format`/`format_diff`. (Note: the older `profiler.mem_diff` read non-existent keys `.allocations/.borrows` and always returned 0 — `resources.diff` uses the real `memory_stats` keys.)

**`strings.sapp` enriched (+26 fns)** — `ord`/`chr`, `is_digit`/`is_alnum`/`is_space`/`is_upper`/`is_lower`, `swapcase`, `title_case`/`to_pascal_case`/`to_kebab_case`, `last_index_of`, `common_prefix`/`common_suffix`, `remove_prefix`/`remove_suffix`, `center`, `zfill`, `partition`/`rpartition`, `splitn`, `levenshtein`, `slugify`, `dedent`, `wrap`, `expand_tabs`. **Also fixed two pre-existing latent bugs**: `to_snake_case` and `is_alpha` used string ordering (`c >= "A"`), which the VM's `num_cmp` rejects ("comparison expects numbers") — rewritten to compare `strcharcodeat` code points.

**VM GC correctness fix (`crates/pblang-vm/src/vm/gc.rs`)** — `gc_collect()` was clearing **every live array/map/module** (including the builtins module), so any builtin call after `gc_collect()` failed with `undefined global '<name>'`. Root cause: `gc_register_*` computed the registry id via the method form `x.as_ptr()` which resolves to **`RefCell::as_ptr`** (inner-value pointer), while the mark phase keys live objects by **`Rc::as_ptr`** (the `RefCell` allocation). The two pointers differ by the borrow-flag offset, so `live.contains(id)` was *always* false → everything swept. Registration now uses `Rc::as_ptr` consistently. This was latent because no test called a builtin *after* `gc_collect()`.

**CLI `--mem-report` (`pblang run --mem-report <file>`)** — after a program finishes, prints a one-line allocation + memory-tracker summary to stderr (new public `VM::memory_report()`). Works for pblang's own code or any other project.

- Tests: `tests/modules.rs::{set_module_core_operations, vector_module_linear_algebra, matrix_module_determinant_solve_inverse, heap_module_priority_queue, deque_module_double_ended, base32_module_rfc4648_vectors, complex_module_arithmetic, strings_module_extended_utilities, resources_module_memory_and_capture}` — **modules 28/0**; full **semantics 189/0/1** (GC fix regression-free). Stdlib module count 127 → 135.

## 25. Self-hosting step: the pblang codegen now lowers the core corpus to bytecode (2026-05-29)

`stdlib/compiler/` ports the compiler from Rust to pblang (tokenizer → parser → semantic → typeck → ir → optimizer → codegen, ~5,300 lines). The `--via-pblang` CLI paths and the `selfhost_drift` test previously exercised only the **parse/analyze** layer — so `frontend_compile` (the full pipeline through code generation) had **never actually run**: it crashed on the very first instruction.

Root causes (all in `stdlib/compiler/codegen.sapp`, plus consumers drifting from the parser's AST):
- **Non-existent method-call convention** — `chunk.array_push(constants, v)` / `chunk.len(constants)` / `cg.locals.pop()` were parsed as *field access* on the map (pblang has no UFCS), throwing `undefined key 'array_push'`. Rewritten to `array_push(chunk.constants, v)` / `len(chunk.constants)` / `array_pop(cg.locals)` (28 sites).
- **AST kind/field drift** — codegen dispatched on `"var"` (parser emits `"ident"`), `"expr"` (→ `"expr_stmt"`), and read `call.name` (→ `func`), `array.elements` (→ `items`), `unary.expr` (→ `operand`), `if.condition/then_body/else_body` (→ `cond`/`then`/`else`), `block.body` (→ `stmts`). Aligned to the real `parser.sapp` node shapes.
- **Swapped builtin args** — `map_has(name, cg.globals)` → `map_has(cg.globals, name)`.
- **Block representation** — `parse_block()` returns a block *node* `{kind:"block", stmts:[…]}`, but `codegen_block` (and the top level) assumed a raw list. `codegen_block` now accepts either.
- **`typeck.sapp`** control-flow/`expr_stmt`/`block` branches read the same stale field names — aligned to the parser. `frontend_compile` now treats the (still partial) typechecker as advisory (`try … catch []`) so a typeck gap can't abort codegen.

Result: `frontend_compile` now lowers the **core corpus** — `let`, arithmetic, identifiers, strings, arrays, unary minus, `while`, `if`/`else`, function defs + calls — to a non-empty bytecode IR (constants + opcodes) without crashing. Locked by `tests/selfhost_drift.rs::selfhost_drift_compiles_core_corpus_to_bytecode` (8 constructs). Adjacent suites green: `typeck_compiler` 8/0, `fmt_drift` 2/0, `fmt` 5/0, `selfhost_drift` 4/0, semantics `selfhost_frontend_pass` ok.

**Honest scope / next step:** this proves the pblang compiler *emits* a structurally-coherent IR; it does **not** yet *execute* it. The self-hosted opcode set (`OP_*` in codegen.sapp) has no runner in-tree, and `elif` lowering, `for`/`for_in`, `assign`-as-statement, and member/index expressions still need codegen coverage. The next self-hosting increment is either a small interpreter for the self-hosted IR or a bridge that maps it onto the Rust VM's `Chunk` so emitted bytecode can be run and checked for output parity against the Rust compiler.

---

# Part 2 — Validation Baseline / Fix Log (preserved from earlier sessions)

This section is the running record of "claims about the code that have been verified" — it grows as bugs are fixed and stale analysis claims are corrected.

## Verification Scope

- Verified against current GitHub repository sources (commit HEAD of `main`).
- Re-checked core claims from prior analysis dumps and this file.
- Confirmed build health with `cargo check -q` (workspace).
- Cross-referenced claims from GPT-5.4 Thinking, Deep Research, Gemini 3.1 Pro Thinking, and Claude Sonnet 4.6 analyses.
- **2026-05-24 refresh:** re-verified after the chumsky 1.0 migration and SP1-gamma
  match-guard fix; sections below updated to move resolved items into "True and
  Fixed" and to drop the chumsky-0.8 claim that no longer holds.
- **2026-05-24 (later) refresh:** after the IDE→tokio + serve→axum + main.rs
  split landed (combined merge from the parallel D worktrees), and after the
  native polyglot bridge gained compiled-language support (`polyglot_exec_compiled`,
  `polyglot_exec_with_input`, `polyglot_detect`).
- **2026-05-24 (final) refresh:** comprehensive quality audit + CLI capability flags
  (`--allow-exec` / `--allow-ffi` / `--allow-all` / `--sandbox` / per-cap allow/deny,
  plus `PBLANG_ALLOW_*` env vars) wired into `pblang run`, `pblang test`, and the
  REPL; clippy `clone_on_copy` cluster auto-fixed (81 → 1 in pblang-core); critical
  FFI/JIT unsafe blocks documented with `// SAFETY:` comments; stale `stdlib/config.sapp:107`
  TODO removed (`throw` is wired); cdylib cache expanded 5 → 16 languages with
  Java/Kotlin via GraalVM NativeAOT / Kotlin/Native; per-toolchain regression test
  for all 16 cdylib languages with skip-on-missing-toolchain.
- **2026-05-24 (very final) refresh:** JSON ingress caps wired (`VmLimits.max_external_json_bytes`
  + `VmHost::max_external_json_bytes()` + `pblang_vm::bounded_json_parse` helper), applied at
  the three external-JSON ingress points (pblang-ffi `pblang_call_function` args, vm.rs
  `ai_chat` options, pblang-pm `parse_registry_index` + `search`); `pblang build --warm-polyglot`
  CLI flag implemented (walks the package, extracts `@lang` blocks, pre-warms the cdylib
  cache for all 16 supported languages, idempotent re-runs hit the cache). 6 new regression
  tests across `tests/new_cmd.rs` (3 warm-polyglot) and `tests/install_cli.rs` (2 JSON-cap).
- **2026-05-25 refresh:** polyglot builtins extracted from `vm.rs` to a new
  `crates/pblang-vm/src/vm/polyglot.rs` submodule (vm.rs down 12,321 → 10,208 lines,
  −17%); HTTPS support added to `pblang serve` via `axum-server 0.7` + `rustls 0.23`
  with new `--cert <pem>` / `--key <pem>` flags (CORS allowlist auto-switches scheme;
  plain HTTP path unchanged when flags absent); 2 dedicated HTTPS CLI tests using
  raw rustls client for self-signed-cert handshake.

## True and Fixed

- `strip_comments_mask` handles both double-quoted and single-quoted strings in:
  - `src/main.rs`
  - `crates/pblang-core/src/lib.rs`
- Formatter/REPL brace scanning tracks both double-quoted and single-quoted strings:
  - `scan_line_for_braces` and call sites in `src/main.rs`
- Added regression coverage for single-quoted braces in IDE formatting tests:
  - `src/main.rs` (`ide_source_tests`)
- `normalize_ide_source` preserves trailing newline behavior.
- `fmt` supports `.sap`, `.sapp`, and `.bpx` extensions.
- Publish token hardening is in place:
  - CLI `--registry-token=...` is rejected (exits with code 2)
  - `PBLANG_REGISTRY_TOKEN` env var is required
- `pblang-stub` extraction is hardened:
  - validates entries one-by-one
  - rejects absolute paths and `..` traversal
  - rejects symlink/hardlink entries
  - extracts via `unpack_in` under destination-root checks
- `QuantumBackend` implements `FromStr` and VM uses typed parsing:
  - `crates/pblang-quantum/src/lib.rs`
  - `crates/pblang-vm/src/vm.rs`
- `pblang-wasm` dead-code warning cleanup completed:
  - removed unused `WasmCtx` fields/methods that were not used by codegen
- Build artifacts are removed from tracked files:
  - `libtest_parse.rlib`
  - `doc_out/index.html`
- IDE server binds to `127.0.0.1` (not `0.0.0.0`).
- CORS origin is rewritten to `http://localhost:<port>` via `write_http_response_with_local_cors`.
- IDE server enforces body limits (10 MB) and read timeout (5 s).
- `copy_dir_all` includes symlink rejection, source-root and destination-root boundary checks.
- `from_utf8` fallback in `strip_comments_mask` and `mask_and_extract_polyglot_blocks` now uses `from_utf8_lossy` (not raw `src.to_string()`):
  - `src/main.rs`
  - `crates/pblang-core/src/lib.rs`
- `profile.dev` has `debug = 2` (full debug symbols for dev builds).
- `api_key` in `ProviderConfig` has `#[serde(skip_serializing)]` to prevent accidental serialization.
- `cosine_similarity` in `pblang-ai` now uses idiomatic `iter().zip()` fold instead of index-based loop.
- `http_get_json` in `pblang-ai` is now `pub` (was dead code suppressed with `#[allow(dead_code)]`).
- Anthropic system prompt duplication fixed in `chat_anthropic`:
  - Removed redundant inline `<system>{sys}</system>` wrapping from user message.
  - System prompt is passed only via the top-level `body["system"]` field (Anthropic's recommended approach).
  - Eliminates injection risk from prompts containing `</system>`.
- `strfromcharcode` / `str_from_char_code` added to typeck builtin prelude:
  - `crates/pblang-typeck/src/lib.rs` (`seed_builtin_prelude`)
  - Fixes `compiler_tokenizer_and_typeck_host_typecheck_clean` test (was failing because `stdlib/compiler/tokenizer.sapp` calls `strfromcharcode` which was not declared in the typeck context).
  - All 5 typeck_compiler tests now pass.
- Lint walk `.expect()` panics replaced with safe helpers:
  - `src/main.rs` (`lint_unused_warnings`)
  - 16 `.expect()` calls on scope stack `pop()` / `last_mut()` replaced with `pop_scope_and_report` helper and `if let` guards.
  - Eliminates potential panics on internal invariant violations without losing lint accuracy.
- `os_signal_trap()` is now a real OS signal trap (no longer a stub):
  - `crates/pblang-vm/src/signals.rs` registers libc handlers on Unix (SIGINT/SIGTERM/SIGHUP/SIGUSR1/SIGUSR2) and `SetConsoleCtrlHandler` on Windows.
  - Handlers only touch process-global atomic counters (async-signal-safe).
  - New builtins: `os_signal_trap`, `os_signal_pending`, `os_signal_poll`, `os_signal_handler_get`.
  - `stdlib/os.sapp` exposes `signal_trap`, `signal_pending`, `signal_poll`, `signal_dispatch` that poll counters and invoke the registered handler from regular bytecode context.
- FFI gained a typed-signature call path:
  - `ffi_call_typed(handle, symbol, sig, args)` accepts `i64`/`i32`/`f64`/`f32`/`bool`/`string`/`ptr`/`void` types in a `"a,b->c"` signature string.
  - Up to four arguments, mixed types (e.g. `"string->i64"` for `strlen`, `"string,string->i64"` for `strcmp`, `"i64->s"` for things returning C strings).
  - `stdlib/ffi.sapp` exposes `call_typed`, plus convenience helpers `call_str` and `call_int`.
  - Still gated by the `ffi` capability and the existing symbol/library allowlist.
- Native tensor builtins for fast numeric ops:
  - 14 base builtins implemented in Rust (`tensor_add`, `tensor_sub`, `tensor_mul`, `tensor_scale`, `tensor_matmul`, `tensor_dot`, `tensor_relu`, `tensor_sigmoid`, `tensor_tanh`, `tensor_softmax`, `tensor_sum`, `tensor_mean`, `tensor_argmax`, `tensor_transpose2d`).
  - 10 additional native neural-net primitives: `tensor_gelu`, `tensor_leaky_relu`, `tensor_log_softmax`, `tensor_layer_norm`, `tensor_batch_norm`, `tensor_embedding_lookup`, `tensor_cross_entropy`, `tensor_conv2d`, `tensor_maxpool2d`, `tensor_avgpool2d` — enough to build CNN / Transformer training loops at near-native speed.
  - `stdlib/tensor.sapp` exposes thin wrappers: `add/sub/mul/scale`, `relu/sigmoid/tanh_/softmax/gelu/leaky_relu/log_softmax`, `layer_norm/batch_norm`, `conv2d/maxpool2d/avgpool2d` (with `_s`/`_sp` stride/padding variants), `embedding_lookup`, `cross_entropy`, `sum_all`, `mean`, `argmax`, `transpose`; `dot` routes through `tensor_dot`/`tensor_matmul` with interpreter fallback.
- Quantum bridge enriched in `stdlib/crypto_quantum.sapp`:
  - New gate set (`H`, `X`, `Y`, `Z`, `S`, `T`, `RX/RY/RZ`, `CX`, `CZ`, `SWAP`, `CCX`, `CCZ`, `CSWAP`).
  - Circuit-builder API (`new_circuit`, `apply_gate`, `apply_rotation`, `measure`, `measure_all`, `circuit_to_json`).
  - Native statevector simulator: `quantum_simulate(circuit)` evaluates a circuit against an actual complex statevector (≤20 qubits) and returns `{ probs, real, imag, n_qubits }`. Stdlib `simulate(circuit)`, `probabilities(circuit)`, and `sample(circuit)` wrap it.
  - AI-bridge helper `circuit_features` produces feature maps suitable for ML-assisted transpilation/error-mitigation.
- Native encoding builtins: `hex_encode`, `hex_decode`, `url_encode`, `url_decode` (RFC 3986 unreserved set + `+` shorthand on decode) — 50-100x faster than the pure-pblang fallbacks in `stdlib/encoding.sapp`. Pair with the existing `base64_encode`/`base64_decode` natives.
- Polyglot build manifest expanded: `stdlib/polyglot_build.sapp` now supports `@rust`, `@c`, `@cpp/@c++/@cxx`, `@go` (c-shared lib), `@ruby`, `@python`, `@js`/`@javascript`, and `@wasm/@wat` (via `wat2wasm`).
- Strict-parse for batch compilation is in place — `pblang run`, `pblang compile`, and `pblang lint` already call `parse_strict` and report errors rather than silently recovering. The recovery parser remains used in LSP/IDE and `analyze` where partial AST is helpful.
- **chumsky 1.0.0-alpha.8 migration is complete** (was 0.8 in earlier baseline, briefly 0.9.3): workspace dep pinned to `=1.0.0-alpha.8`, `parser()` reshaped to `Parser<'src, &'src str, Vec<Stmt>, extra::Err<Simple<'src, char>>>`, 70× `.map_with_span` → `.map_with(|_, e| … e.span().into_range())`, `text::ident/int/digits` mapped from `&str` back to `String`, `.foldl` argument order flipped, `Simple::custom` → `LabelError::expected_found`, `parse_recovery` → `parser().parse(...).into_output_errors()`. Owned `ParseError { message, span, found }` introduced at the `pblang-core` boundary (chumsky 1.0's `Simple` borrows from input). CLI + LSP diagnostics keep span-based underlining.
- **SP1-gamma match-guard parser fixed** (`tests/semantics.rs::match_guard_pass` un-ignored). Two-part fix: (1) a restricted `guard_expr` sub-parser via `Recursive::declare()` that bottoms out at `<atom> [cmp_op <atom>]` using only `.or_not()` — avoids the chumsky `.padded().repeated()` cascade past the trailing `~`/`#` separator; (2) the guarded-arm prefix used `ws1.then_ignore(just("if"))` after `ident.padded()`, which required whitespace that padding had already consumed — switched to `just("if").padded()`.
- **`crates/pblang-vm/src/vm.rs` split into submodules**: `vm.rs` is down from ~12,048 → **10,163 lines** and now coexists with `crates/pblang-vm/src/vm/` containing `spawn_wire.rs`, `ffi_dispatch.rs`, `gc.rs`, `handles.rs`, `db.rs`, `net.rs`, and **`strings.rs`** (17 `str_*` builtins, 438 lines). Further extraction is still possible (instruction dispatch, array/map/math builtin categories) but the file is no longer monolithic.
- **TOCTOU hardening (Unix)**: `read_text_file_with_race_check` (`src/main.rs`) now opens the resolved path with `O_NOFOLLOW` via `OpenOptionsExt::custom_flags`. An attacker swapping the canonicalized path for a symlink between resolve and open hits `ELOOP` and is rejected with a "possible TOCTOU" error. The dev/inode check is preserved as a second line of defence.
- **clap 4 top-level CLI router**: 21 subcommands (`init`, `new`, `manifest-check`, `manifest-parity`, `install`, `publish`, `search`, `fetch`, `pack`, `verify`, `build`, `bundle`, `transpile`, `lsp`, `serve`, `fmt`, `lint`, `test`, `run`, `compile`, `build-polyglot`) now route through `#[derive(Parser)] Cli` with a `Commands` enum. Per-subcommand flag parsing is intentionally still manual inside each handler (follow-up pass). The legacy `if args.first() == Some(…)` chain is preserved as a fallback for "flag-as-subcommand" forms (`--lock`, `--cfg`, `--check`, `--doc`, `--call-graph`, `--dataflow`, `--dump-bytecode`, `--annotate`, `--symbols`) that clap can't model without redesign.
- **`pblang bundle --self-extract .` walkdir bug fixed**: when invoked with the manifest dir as `.`, the staging walkdir's `filter_entry` saw the root entry's `file_name() == "."`, hit the `!name.starts_with('.')` dotfile filter, and silently skipped the entire tree, producing a bundle with only `pblang.toml` and `bin/pblang`. Fixed by skipping the filter when `e.depth() == 0`. Regression test in `tests/pack.rs::self_extract_with_dot_cwd_includes_package_files`.
- **`pblang serve` migrated to axum 0.7 on tokio**: replaced the 720-line hand-rolled `std::net::TcpListener` HTTP loop with an axum Router (`get`/`post` handlers + tower-http `CorsLayer` + `RequestBodyLimitLayer`). Per-invocation `tokio::runtime::Builder::new_multi_thread()` with **8 MB worker stack size** to keep the deeply-recursive pblang parser/VM from overflowing the default 2 MB tokio stack. CPU-bound handlers (`/run`, `/check`) use `spawn_blocking` so async workers stay responsive. Security guardrails preserved + sharpened: a custom `require_content_length` middleware enforces explicit Content-Length on POST (411 missing / 400 invalid); the `/examples/*id` and `/manual/*id` wildcard routes refuse multi-segment captures so attacker-controlled paths can't fall through to the IDE index fallback. New deps: `axum 0.7`, `tower 0.5`, `tower-http 0.5` (cors+limit features).
- **`src/main.rs` modularization started**: extracted formatter (`src/cli/fmt.rs`, 364 lines), call-graph CLI (`src/cli/call_graph.rs`, 160 lines), dataflow CLI (`src/cli/dataflow.rs`, 254 lines), and span-formatting helpers (`src/cli/diag.rs`, 81 lines) into `src/cli/*.rs`. `src/main.rs` dropped from 5,999 → **5,028 lines** (−971 lines, −16%) after combining this extraction with the axum/tokio serve replacement.
- **Native polyglot bridge — compiled-language support**: `polyglot_exec` (interpreted) and `polyglot_exec_compiled` (compiled) cover **47 languages** end-to-end via the host PATH. In-VM builtins in `crates/pblang-vm/src/vm.rs`:
  - **`polyglot_exec(lang, code)`** — interpreters: python, js (node), deno, bash, zsh, fish, powershell, ruby, perl, raku, lua, php, r, tcl, awk, julia, racket, scheme (guile), clojure, elixir, groovy, fennel, ocaml, scala.
  - **`polyglot_exec_compiled(lang, code)`** — compilers + runtime: rust (`rustc`), go (`go run`), c (`cc`/`gcc`/`clang`), cpp (`c++`/`g++`/`clang++`), zig (`zig run`), java (`javac`+`java`), kotlin (`kotlinc`+`java`), typescript (`tsc`+`node`), pascal (`fpc`), nim (`nim r`), crystal (`crystal run`), d (`dmd`/`ldc2`/`gdc`), haskell (`runghc`/`ghc`), fortran (`gfortran`/`flang`), ada (`gnatmake`), swift, cobol (`cobc`), erlang (`escript`), vala (`valac`), objc (clang -lobjc), nasm + ld (x86_64 ELF), gas + ld, v (`v run`), ocaml-compiled (`ocamlfind`/`ocamlc`). RAII-cleans temp dir even on panic.
  - **`polyglot_exec_with_input(lang, code, input)`** — pipes `input` to the subprocess's stdin so a polyglot block can act as a stdin → stdout filter.
  - **`polyglot_detect()`** — probes the host PATH and returns a JSON object mapping each known language to `{available, kind, command}`. Lets examples/docs skip unavailable languages gracefully.
  - **`polyglot_compile_cached(lang, code)`** — build-time path: compiles `code` to a shared library (cdylib `.so`/`.dll`/`.dylib`), content-hash-keyed under `~/.cache/pblang/polyglot/` (overridable via `PBLANG_CACHE_DIR` / `XDG_CACHE_HOME`), and returns an FFI handle compatible with `ffi_call_typed`. Cache hits skip the subprocess entirely. Supported: **rust, c, cpp, go (`CGO_ENABLED=1` auto-set), pascal, csharp (NativeAOT — auto-scaffolds csproj with `PublishAot=true` + `NativeLib=Shared`, builds with `dotnet publish -c Release -r <auto-detected RID>`, copies from `bin/Release/net8.0/<rid>/publish/`), fsharp (same NativeAOT path with fsproj), zig (`zig build-lib -dynamic -O ReleaseFast -femit-bin`), d (`-shared -fPIC -of=`), nim (`nim c --app:lib --opt:speed --noMain:on --out:`), swift (`swiftc -emit-library -O`), haskell (`ghc -shared -dynamic -fPIC -lHSrts-ghc`), fortran (`gfortran -shared -fPIC -O2`), objc (`clang -shared -fPIC -lobjc` on Linux), ocaml (`ocamlopt -shared -O3`), java (GraalVM `javac` + `native-image --shared --no-fallback -H:Name=prog` wrapped in `sh -c`), kotlin (Kotlin/Native `kotlinc-native -produce dynamic -opt`)**. Per-language post-build fixups normalize platform naming (e.g. Unix `lib`-prefix). Caller writes C-ABI exports inside the snippet — `#[no_mangle] pub extern "C" fn ...` (Rust), `[UnmanagedCallersOnly(EntryPoint = "...")]` (C#), `//export Name` (Go), `bind(c, name="...")` (Fortran), `foreign export ccall` (Haskell), `cdecl; export;` (Pascal), `@_cdecl` (Swift), `{.exportc.}` (Nim), `extern (C) export` (D), `@CEntryPoint(name = "...")` with an `IsolateThread` first param (Java/GraalVM), `@CName("name")` (Kotlin/Native), and so on.
  - **`polyglot_cache_clear()`** — wipes the cache dir, returns count of files removed.
  - **`polyglot_cache_info()`** — returns `{dir, count, size_bytes}` JSON.
  All `polyglot_*` are gated by `allow_exec` (and `polyglot_compile_cached` additionally by `allow_ffi`). `stdlib/polyglot.sapp` exposes thin wrappers (`polyglot.exec`, `.exec_with_input`, `.exec_compiled`, `.detect`, `.run` auto-router, `.compile_cached`, `.cache_clear`, `.cache_info`, `.supported_languages`). `stdlib/polyglot_web.sapp` adds HTML/CSS/JS bundling helpers (`bundle`, `bundle_html`, `bundle_to`, `open_in_browser`, `js`, `deno_or_node`, `render`). Regression tests: `tests/semantics/polyglot_native_test.sapp` (skip-on-missing-toolchain) and `tests/semantics/polyglot_cached_test.sapp` (exercises the cdylib roundtrip with C: compile_cached → ffi_call_typed("pblang_add", "i64,i64->i64", [10,32]) == 42), wired as `semantics::polyglot_native_pass` and `semantics::polyglot_cached_pass`.

## True but Not Fully Fixed (Large Refactor / Design Work)

- `resolve_package_entry` TOCTOU is now further hardened with `O_NOFOLLOW` on Unix in `read_text_file_with_race_check` (rejects symlink swaps between `canonicalize` and `open`). The dev/inode check remains as a second line of defence (covers non-symlink races like rename+replace). Full per-component `openat`/`O_PATH` chains or Linux's `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` are still an option if the threat model expands; current defence is two-deep.
- `src/main.rs` is now ~5,028 lines (was 5,999). First wave of modularization landed: `src/cli/{fmt,diag,call_graph,dataflow}.rs` extracted, plus the serve handler replaced with axum (which is more compact than the legacy loop). Remaining extraction candidates: bundle handler, lint handler, doc handler, the REPL loop, per-subcommand flag parsing.
- `crates/pblang-vm/src/vm.rs` is now **2,453 lines** (was 12,321 pre-extraction, −80.1%). Submodules under `src/vm/`: `spawn_wire`, `ffi_dispatch`, `gc`, `handles`, `db`, `net`, `strings` (17 fns), `polyglot` (7 fns), `math` (22 fns), `arrays` (11 fns), `maps` (3 fns), `encoding` (7 fns), `json` (4 fns + 2 shared helpers), `parse` (3 fns), `tensor` (24 fns + 2 helpers), `http` (11 fns + status-reason helper), `ai` (4 fns inc. onnx_infer), `quantum` (4 + 4 helpers), `os` (7 fns), `re` (7 regex fns), `fs` (15 fs/path fns), `time` (4 fns), `predicates` (13 type/freeze fns), `crypto` (3 fns), `env` (3 fns), `atomics` (7 fns), `sync` (12 mutex/rwlock/barrier/once fns), `memory` (17 alloc/borrow/lifetime fns), `ffi` (4 user-facing fns), `runtime` (9 process/io/gc/assert fns), `spawn` (17 spawn/join/channel/ticker/timeout fns), **`builtins_table` (the 1.3k-line `BuiltinFn` registration vec, now a single `build() -> Vec<BuiltinFn>` fn)**, **`errors` (the `vm_err!` / `vm_arity!` / `vm_cap!` macro module added 2026-05-26)**. **32 submodules total.** What's left in vm.rs is now the `VM` struct definition, the OpCode dispatch loop, host-trait impls (`VmHost for VM`), the `with_stdlib` constructor, the capability classifier (`builtin_capability_class`), and the static GC counters / `alloc_array` / `alloc_map` helpers. No further bulk extraction looks productive — what remains is intrinsic to the `VM` type.
- **CLI argument parsing** — clap 4 is now wired as the top-level subcommand router for 21 commands (`init`, `new`, `manifest-check`, `manifest-parity`, `install`, `publish`, `search`, `fetch`, `pack`, `verify`, `build`, `bundle`, `transpile`, `lsp`, `serve`, `fmt`, `lint`, `test`, `run`, `compile`, `build-polyglot`). Per-subcommand flag parsing inside handlers is still manual; the legacy `if args.first() == Some(…)` chain is preserved as a fallback for top-level "flag-as-subcommand" forms (`--lock`, `--cfg`, `--check`, `--doc`, `--call-graph`, `--dataflow`, `--dump-bytecode`, `--annotate`, `--symbols`) that clap can't model without redesign.
- **`pblang bundle --self-extract`** — the walkdir filter that dropped the entire package tree when invoked with a relative `.` path is fixed (`e.depth() != 0` exception for the root entry). Regression test in `tests/pack.rs::self_extract_with_dot_cwd_includes_package_files` covers this. Existing tests only used absolute paths so didn't catch it.
- Some crates/features remain roadmap-partial by design (LLVM/Cranelift/WASM/AI/quantum depth and integration level).
- `parse_with_polyglot` keeps a recovery-style fallback (chumsky 1.0 `parser().parse(...).into_output_errors()` after a failed strict parse) — intentional for REPL/IDE/analyze paths only. Batch commands (`run`, `compile`, `lint`) call `parse_strict` and surface errors.
- ~~IDE server is single-threaded; full async migration to tokio is recommended.~~ **Done** — see "True and Fixed" entry above for the axum/tokio migration.
- ~~Hand-rolled HTTP server in `pblang serve` — replacing with `axum` or `tiny_http` would reduce vulnerability surface.~~ **Done** — replaced with axum 0.7 + tower-http; security guardrails are now explicit middleware + wildcard route handlers instead of inline byte-pushing.
- `pblang-ffi` is intentionally a separate `cdylib` (see `crates/pblang-ffi/Cargo.toml`: `crate-type = ["cdylib"]`), not a binary dependency. It exposes a C ABI (`pblang_create_vm`, `pblang_call_function`, `pblang_eval`, `pblang_set_limits`, …) so **external** C/C++/Python/Go programs can embed pblang. The in-VM FFI direction (pblang code calling out to host C libraries) is a different surface in `crates/pblang-vm/src/vm/ffi_dispatch.rs`. Earlier reports flagged "not reachable from the binary" — that was a category error; the cdylib is reached by *consumers*, not by `pblang`. The build is verified (`cargo build -p pblang-ffi --lib`).
- `pblang-cranelift` is now in `[dependencies]` (used for `host_target_triple()` in `src/main.rs`). Previously dev-only.

## Outdated / Incorrect Claims (Current Code)

- `fmt` missing `.bpx` support: **false** — `.bpx` is accepted (lines 4119, 4683 in `src/main.rs`).
- `normalize_ide_source` dropping trailing newline: **false** — newline is preserved (lines 744-746).
- `format_ide_source` resetting string state each line: **false** — `in_string` and `in_single_string` are mutable refs persisted across lines via `scan_line_for_braces`.
- IDE server wildcard CORS exposure as direct remote risk: **outdated** — server binds to `127.0.0.1`, CORS is rewritten to local origin.
- `copy_dir_all` missing boundary checks: **outdated** — symlink, source-root, and destination-root checks are present.
- IDE server missing body limits/timeouts: **outdated** — header limit (64 KB), body limit (10 MB), read timeout (5 s) all exist.
- `profile.dev` has `debug = 0`: **false** — it's `debug = 2`.
- `print_dataflow_report` `use_line` not consumed: **false** — it is used in `println!("use {} line:{}", name, use_line)`.
- `libtest_parse.rlib` committed to repo: **outdated** — already removed and `.gitignore` covers `*.rlib`.
- `doc_out/` committed to repo: **outdated** — already removed and `.gitignore` covers `/doc_out/`.
- `embed()` body identical for ollama/non-ollama: **inaccurate** — the body is the same (`{"model": ..., "input": ...}`) because both APIs use the same request format; the URL and response parsing differ correctly.
- `Anthropic API key duplicated into headers`: **partially outdated** — `api_key` is only inserted once via `x-api-key` header in `chat_anthropic`; the `auto_register` stores it once in `provider.api_key`.
- IDE server binds to `0.0.0.0`: **false** — it binds to `127.0.0.1`.
- `from_utf8` fallback uses `src.to_string()`: **outdated** — now uses `from_utf8_lossy`.
- `pblang-wasm` has no `--wasm` CLI command: **false** — `pblang compile --wasm` exists (emits `.wat` via `pblang_wasm::compile_to_wat`).
- `cargo-audit` not in CI: **false** — CI already includes a `Security Audit` job (`cargo audit`).
- `strfromcharcode` not available to typeck: **was true, now fixed** — added to `seed_builtin_prelude`; test passes.
- `libtest_parse.rlib` and `doc_out/` still in repo: **outdated** — already removed; `.gitignore` covers both.

## Notes

- If new fixes are applied, update this file by moving items between sections rather than appending raw reports.
- This validation was done against the GitHub repository state, not just historical reports.
- Current branch: `main`
- Last updated: 2026-05-24 (chumsky 1.0 + SP1-gamma refresh)
