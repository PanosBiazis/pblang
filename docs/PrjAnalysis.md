# pblang Analysis Validation (May 2026)

This document is the current validation baseline for analysis claims against the local workspace code.

## Verification Scope

- Verified against current GitHub repository sources (commit HEAD of `main`).
- Re-checked core claims from prior analysis dumps and this file.
- Confirmed build health with `cargo check -q` (workspace).
- Cross-referenced claims from GPT-5.4 Thinking, Deep Research, Gemini 3.1 Pro Thinking, and Claude Sonnet 4.6 analyses.
- **2026-05-24 refresh:** re-verified after the chumsky 1.0 migration and SP1-gamma
  match-guard fix; sections below updated to move resolved items into "True and
  Fixed" and to drop the chumsky-0.8 claim that no longer holds.

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
- **`crates/pblang-vm/src/vm.rs` split into submodules**: `vm.rs` is down from ~12,048 → 10,586 lines and now coexists with `crates/pblang-vm/src/vm/` containing `spawn_wire.rs`, `ffi_dispatch.rs`, `gc.rs`, `handles.rs`, `db.rs`, `net.rs`. Further extraction is still possible (instruction dispatch, builtins by category) but the file is no longer monolithic.

## True but Not Fully Fixed (Large Refactor / Design Work)

- `resolve_package_entry` TOCTOU is now further hardened with `O_NOFOLLOW` on Unix in `read_text_file_with_race_check` (rejects symlink swaps between `canonicalize` and `open`). The dev/inode check remains as a second line of defence (covers non-symlink races like rename+replace). Full per-component `openat`/`O_PATH` chains or Linux's `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` are still an option if the threat model expands; current defence is two-deep.
- `src/main.rs` is now ~5,999 lines (grew slightly when the clap migration added 21 subcommand wrappers + `legacy_dispatch`). Modularization into `src/cli/*.rs` modules is still substantial architectural work; the clap migration is a prerequisite that's now done.
- `crates/pblang-vm/src/vm.rs` is now 10,163 lines (was 10,586 before string-builtins extraction). Submodules under `src/vm/`: `spawn_wire`, `ffi_dispatch`, `gc`, `handles`, `db`, `net`, **`strings` (new — 438 lines, 17 str_* builtins)**. Further extraction candidates: array builtins, map builtins, math builtins, and the main instruction dispatch.
- **CLI argument parsing** — clap 4 is now wired as the top-level subcommand router for 21 commands (`init`, `new`, `manifest-check`, `manifest-parity`, `install`, `publish`, `search`, `fetch`, `pack`, `verify`, `build`, `bundle`, `transpile`, `lsp`, `serve`, `fmt`, `lint`, `test`, `run`, `compile`, `build-polyglot`). Per-subcommand flag parsing inside handlers is still manual; the legacy `if args.first() == Some(…)` chain is preserved as a fallback for top-level "flag-as-subcommand" forms (`--lock`, `--cfg`, `--check`, `--doc`, `--call-graph`, `--dataflow`, `--dump-bytecode`, `--annotate`, `--symbols`) that clap can't model without redesign.
- **`pblang bundle --self-extract`** — the walkdir filter that dropped the entire package tree when invoked with a relative `.` path is fixed (`e.depth() != 0` exception for the root entry). Regression test in `tests/pack.rs::self_extract_with_dot_cwd_includes_package_files` covers this. Existing tests only used absolute paths so didn't catch it.
- Some crates/features remain roadmap-partial by design (LLVM/Cranelift/WASM/AI/quantum depth and integration level).
- `parse_with_polyglot` keeps a recovery-style fallback (chumsky 1.0 `parser().parse(...).into_output_errors()` after a failed strict parse) — intentional for REPL/IDE/analyze paths only. Batch commands (`run`, `compile`, `lint`) call `parse_strict` and surface errors.
- IDE server is single-threaded (uses `threadpool` for request handling but still one `TcpListener::incoming()` loop); full async migration to tokio is recommended. **Ready for a fresh session** — bounded scope, IDE handler is ~200 lines of accept-loop + response writing.
- Hand-rolled HTTP server in `pblang serve` — replacing with `axum` or `tiny_http` would reduce vulnerability surface. **Ready for a fresh session** — handlers span ~600 lines from line 4042 in `src/main.rs` with manual `HTTP/1.1 …\r\n…` response strings throughout.
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
