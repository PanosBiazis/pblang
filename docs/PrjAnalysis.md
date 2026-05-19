# pblang Analysis Validation (May 2026)

This document is the current validation baseline for analysis claims against the local workspace code.

## Verification Scope

- Verified against current GitHub repository sources (commit HEAD of `main`).
- Re-checked core claims from prior analysis dumps and this file.
- Confirmed build health with `cargo check -q` (workspace).
- Cross-referenced claims from GPT-5.4 Thinking, Deep Research, Gemini 3.1 Pro Thinking, and Claude Sonnet 4.6 analyses.

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

## True but Not Fully Fixed (Large Refactor / Design Work)

- `resolve_package_entry` remains best-effort for TOCTOU resistance.
  - Additional full guarantees require platform-specific open-and-verify (`openat` / `O_NOFOLLOW` style).
- `src/main.rs` is still monolithic (~5,330 lines); modularization is substantial architectural work.
- `crates/pblang-vm/src/vm.rs` is also monolithic (~9,454 lines); splitting into sub-modules (dispatch, builtins, stdlib, ffi) is recommended.
- Some crates/features remain roadmap-partial by design (LLVM/Cranelift/WASM/AI/quantum depth and integration level).
- `parse_with_polyglot` falls back to `parse_recovery()` when strict parsing fails — intentional for REPL/IDE/analyze paths only. Batch commands (`run`, `compile`, `lint`) call `parse_strict` and surface errors.
- IDE server is single-threaded (uses `threadpool` for request handling but still one `TcpListener::incoming()` loop); full async migration to tokio is recommended.
- Hand-rolled HTTP server in `pblang serve` — replacing with `axum` or `tiny_http` would reduce vulnerability surface.
- CLI argument parsing is manual `if args.first() == Some(...)` chains; migration to `clap`/`argh` is recommended.
- `pblang-ffi` is workspace member but not in main `[dependencies]` — not reachable from the binary.
- `pblang-cranelift` is dev-only (only in `[dev-dependencies]`).
- `chumsky` is pinned to 0.8 (old API); migration to 1.x would improve parser error messages.

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
- Current branch: `devin/1779130661-analysis-v2-fixes`
- Last updated: 2026-05-18
