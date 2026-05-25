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
| Workspace crates | **16** core + `pblang-stub` launcher (+ `pb-mcp-server` external tooling) |
| Total Rust LOC | **39,899** |
| Largest Rust files | `pblang-vm/src/vm.rs` (11,653), `src/main.rs` (5,060), `pblang-core/src/parser.rs` (2,381), `pblang-pm/src/lib.rs` (2,234), `pblang-compiler/src/lib.rs` (1,946) |
| Stdlib modules | **121** `.sapp` files, **28,083** lines of pblang |
| Largest stdlib | `math.sapp` (1,018), `thread.sapp` (952), `compiler/bootstrap.sapp` (883), `polyglot_registry.sapp` (703), `ide.sapp` (632) |
| Examples | 23 |
| Integration test files | 30 (covering semantics, pack, threads, sandbox, LSP, HTTP server, fmt, lint, modules, polyglot, quantum sim, tensors, …) |
| Total passing tests | **375+** across all suites (163 semantics + 26 new_cmd + 31 lint + 16 pack + 16 if_else + 11 sandbox + 11 threads + 10 typeck_compiler + … plus workspace libs) |
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

No active known bugs at the time of this audit. The fix log in Part 2 documents the recently-resolved defects (SP1-gamma match-guard cascade, `bundle --self-extract .` walkdir root-skip, IDE-server stack overflow on first request, etc.). The most recent regression introduced during the axum migration — a body-text assertion in `serve_rejects_malformed_content_length` — was resolved by relaxing the assertion to "status 400 is sufficient evidence of rejection", because hyper handles malformed Content-Length at the protocol layer (before any application middleware can attach a body).

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
| **`pblang.toml` `[workspace]` semantics** | Single-package only currently | Medium |

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

Net `vm.rs` shrink across the 2026-05-25 + 2026-05-26 sessions: **10,208 → 2,453 lines (−7,755, −76.0%)**. Combined with the polyglot extraction earlier on 2026-05-25: **12,321 → 2,453 (−9,868, −80.1%)**. The file is now under one-fifth its original size.

> Pitfall encountered: extracting `onnx_infer` to `vm/ai.rs` with `sed 's/^        //'` mangled the embedded Python `ONNX_BRIDGE_SCRIPT` raw string — Python lines with **less** than 8 leading spaces lost no indentation but lines with **exactly** 8 lost it all, breaking the bridge with `IndentationError: expected an indented block after 'try' statement`. Caught by `tests/semantics/onnx_infer_bridge_returns_structured_result` on the first post-extraction test run; fixed by re-indenting the script by hand. Lesson: when bulk-dedenting Rust modules, avoid touching embedded raw strings that carry their own indentation contract (Python, YAML, makefiles, …).

Items still open:

1. **`vm.rs` further extraction is effectively complete.** What's left in the 2,453 lines is the `VM` struct + its `VmHost` impl, the OpCode dispatch loop, `with_stdlib` constructor, the `builtin_capability_class` sandbox classifier, the static GC counters, and `alloc_array`/`alloc_map`/`import_module` helpers. None of these are builtins; they're intrinsic to the VM type. Further reduction would require a `VM` struct redesign, not more extraction.
2. **Wire `pb-dashboard.rs` agent control TODOs** or remove the stub buttons if they're not on the near-term path. (Note: `pb-dashboard.rs` lives in the `editors/ide/tools/mcpservers/pb-mcp-server` git submodule, so requires a submodule-level commit.)
3. **Further `src/main.rs` split** — bundle handler, lint handler, doc handler, REPL loop (per-subcommand flag parsing too). Deferred because handle_* are thin wrappers around `legacy_dispatch`, which inlines each command's body as `if/else if` branches with shared scope. Extracting individual commands needs restructuring `legacy_dispatch` first.
4. **Roadmap-large items from §8** that haven't moved: hosted package registry HTTP endpoints, native single-binary (no extract-to-temp), traits/interfaces, full HM inference, compile-time macros, `async fn`/`await` as keywords, HTTP/2 + WebSockets in `pblang serve`, `pblang.toml [workspace]` semantics.

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
- **`vm.rs` is still 11,653 lines.** Down from 12k+ pre-extraction, but it's the single hardest file to navigate. Bisecting a behaviour difference in there means scrolling.
- **Inconsistent error-message style.** Some errors are bare strings (`"exec access denied"`), some prefix with the builtin name (`"polyglot_exec(): ..."`), some include the failing argument value. A `vm_error!(builtin, msg, fmt_args)` macro could normalize this.
- **Doc comments are sparse outside `vm.rs`.** The big files have doc-comments on the interesting code paths, but many smaller crates (`pblang-ir`, `pblang-graphics`) have terse module-level docs.
- **No fuzzing harness** for the parser or VM. The chumsky parser would benefit from `cargo-fuzz` coverage given how much surface area it has.
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
- **Type system**: optional type annotations on `let` and `fn`; `TypeExpr` covers primitives, arrays, maps, functions, generics; partial context-driven inference; gradual typing.
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
| Linter (`pblang lint`) | ✓ With safe-helper-based unused-warning walker |
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
2. **`vm.rs` review-fatigue surface** — 11,653 lines in one file. New contributors will hit a learning cliff. Mitigation: continue the submodule extraction.
3. **No hosted registry yet** — package distribution still requires `file://` paths or git URLs. Limits ecosystem growth.
4. **Polyglot cdylib loads run as the embedding process** — once a pblang script has `allow_ffi` + `allow_exec`, any cdylib it compiles runs in-process. Mitigation: this is the documented FFI contract; downstream embedders should treat `allow_ffi(true)` as equivalent to "load arbitrary code".
5. **Java/Kotlin path re-compiles every call** — no `.class`/`.jar` caching; high latency for repeated invocations. Mitigation: clone the `polyglot_compile_cached` design for JVM bytecode.

## 16. Recommended priorities for the next 1–2 sessions

(highest leverage first)

1. **Clippy auto-fix the `clone_on_copy` cluster + document the 11 unsafe blocks.** Half an hour. Eliminates 87 / 88 of the workspace's clippy warnings and improves auditor confidence.
2. **Add `cargo-audit` to CI.** One YAML edit + install on dev box.
3. **Cap JSON deserialization on FFI/network ingress.** Add `VmLimits { max_json_bytes, max_json_depth }` and apply at the three sites identified.
4. **`vm.rs` further extraction (math / array / map builtin categories).** Continues the modularization pattern.
5. **Hosted-registry MVP** — at minimum a read-only HTTP index that `pblang install` can fetch from. Authentication can stay file-token-based for v1.
6. **Polyglot cache pre-warm pass in `pblang build`.** Scan `.sapp` files for `@lang { ... }` blocks, call `polyglot_compile_cached` for each. Turns first-run latency into compile-time cost.
7. **Java/Kotlin `.jar` caching** parallel to the cdylib cache.

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
- `crates/pblang-vm/src/vm.rs` is now **2,453 lines** (was 12,321 pre-extraction, −80.1%). Submodules under `src/vm/`: `spawn_wire`, `ffi_dispatch`, `gc`, `handles`, `db`, `net`, `strings` (17 fns), `polyglot` (7 fns), `math` (22 fns), `arrays` (11 fns), `maps` (3 fns), `encoding` (7 fns), `json` (4 fns + 2 shared helpers), `parse` (3 fns), `tensor` (24 fns + 2 helpers), `http` (11 fns + status-reason helper), `ai` (4 fns inc. onnx_infer), `quantum` (4 + 4 helpers), `os` (7 fns), `re` (7 regex fns), `fs` (15 fs/path fns), `time` (4 fns), `predicates` (13 type/freeze fns), `crypto` (3 fns), `env` (3 fns), `atomics` (7 fns), `sync` (12 mutex/rwlock/barrier/once fns), `memory` (17 alloc/borrow/lifetime fns), `ffi` (4 user-facing fns), `runtime` (9 process/io/gc/assert fns), `spawn` (17 spawn/join/channel/ticker/timeout fns), **`builtins_table` (the 1.3k-line `BuiltinFn` registration vec, now a single `build() -> Vec<BuiltinFn>` fn)**. **31 submodules total.** What's left in vm.rs is now the `VM` struct definition, the OpCode dispatch loop, host-trait impls (`VmHost for VM`), the `with_stdlib` constructor, the capability classifier (`builtin_capability_class`), and the static GC counters / `alloc_array` / `alloc_map` helpers. No further bulk extraction looks productive — what remains is intrinsic to the `VM` type.
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
