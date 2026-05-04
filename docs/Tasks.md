Τώρα έχω πλήρη εικόνα. Ιδού το νέο Tasks.md:

text
# Tasks.md — Sapphire Active Backlog

Active tasks only. History belongs in `docs/Report.md`.

**Goal:** Sapphire evolves as a production platform — embedded + standalone,
AI/Quantum/Web/Polyglot-ready, future-proof, no artificial limits, test-driven
steps, stable core. The language aims to be the most powerful, optimal,
easy-to-use, and smart programming language possible.

**Current priority order:**
Web depth → AI/Quantum depth → Executables → Tooling/LSP →
Stdlib/Graphics → Registry → Threads → RE → Spec/Production → Editor → Project/Framework → Native/OS

---

## Global Rules

- Agent may complete **1 primary + up to 3 closely related follow-ups** per run.
- Follow-ups must share the same area, same/adjacent files, and same test/doc update.
- Read `docs/Tasks.md` and `docs/Report.md` before every run.
- No reopening completed items unless: bug fix, regression, explicit extension, or doc correction.
- No uncontrolled refactors — only if they directly unblock the current batch.
- Every behavior change needs tests and must be cross-platform safe.
- **Done = tests pass + no unrelated file changes + Report.md and Tasks.md updated.**

---

## Already Done (do not redo)

> Regressions and extensions are still allowed.

- Semantics suite 54/54, sandboxing/capabilities, VmLimits, FfiPolicy, baseline CI (Linux + Windows)
- Registry M1: checksum, strict mode, `--registry-index`, local publish MVP,
  immutability, `file://` hardening, bare imports, git deps, lockfile
  rehydration, safe tarball unpack, entry validation, subpath confinement,
  `--require-registry-sha256`
- Registry M3: semver resolution (Version::parse, satisfies, ^/~/>=), resolve_registry_version,
  Registry_hosted.md design note (GET/POST/search, bearer auth)
- Registry **M4-alpha**: `fetch_and_install_package`, `sapphire install <name>@<version>`
- Registry **M4-beta**: `publish_package`, `search_registry`, `sapphire publish` (hosted/file),
  `sapphire search <query>`, `publish_and_search_roundtrip` test, Registry_hosted.md M4-beta,
  Tooling.md updated
- Executables E3: `sapphire bundle` + `sapphire pack`, manifest-aware entry
  resolution, packaged app run, path-dep escape rejection, run hint, description display
- Executables E4: `sapphire verify`, HMAC-SHA256 sidecar, install-time HMAC verify,
  manifest metadata (target_platform, min_runtime), validate_manifest_metadata
- Executables **E5-alpha**: `docs/Native_backend_notes.md` Stage A+B design
- Executables **E5-gamma**: self-extracting binary implementation, `self_extract_binary_contains_sentinel` test, `self_extract_runs_hello` test, `bundle --self-extract --output` working: `crates/sapphire-stub/` sentinel stub, `sapphire bundle --self-extract`
  flag accepted, `bundle_self_extract_flag_accepted` test
- Web W2: fixed-length request body, cookie helpers, static file helper, custom
  headers, redirect helper, body-size limit + 413
- Web W3: routing layer (router.sapp), middleware (middleware.sapp), CORS helpers,
  WebSocket upgrade (sha1, websocket_accept_key, upgrade_request/response),
  multipart parse (str_index_of, str_slice, str_lower), chunked_read (W3-delta),
  WebSocket frame parsing — RFC 6455 (strcharcodeat, strfromcharcode, byte_xor, parse_frame W3-epsilon)
- Web **W4-alpha**: `tcp_set_timeout`, `serve_once_timeout`, `serve_forever_timeout`,
  `sse_event`, `res_sse_headers`, `docs/TLS_notes.md`
- Web **W4-beta**: `websocket.sapp` `write_frame`
- Web **W4-gamma**: TLS termination stub (stdlib/tls.sapp, listen_tls, serve_once_tls), tls_stub_test_pass
- Web **W4-beta+**: `http_server.sapp` `mw_rate_limit`
- AI bridge A2: `examples/ai_plugin_minimal/`, bridge contract (plugin_info, capabilities, build_request, invoke)
- AI A3: HTTP provider adapter (ai_http_provider), tensor.sapp (zeros/ones/get/set/shape/dtype),
  http_post_auth builtin + stdlib + openai_chat_auth
- AI **A4-alpha**: `get_stream`, `post_stream`, `parse_sse_line`, openai_chat_stream example
- AI **A4-beta**: `post_stream_auth`, `provider_registry.sapp`
- AI **A4-gamma**: `examples/ai_chat_toolcall/main.sapp` (tool definition schema, `finish_reason == "tool_calls"` parsing, tool execution routing), `stdlib/tensor.sapp` slice/reshape/dot, `tensor_ops_pass` test
- Quantum Q1: `examples/quantum_bridge_minimal/`, job envelope, stdlib/quantum.sapp, stdlib/crypto_quantum.sapp (QKD/BB84)
- Quantum Q2: quantum_sim.sapp (zero_state, h_gate, x_gate, cnot_gate, measure, random_float),
  quantum_protocol.sapp (submit/poll in-memory + submit_remote/poll_remote JSON),
  Quantum_providers.md provider slot design
- Quantum **Q3-alpha**: `run(circ)`, `measurements_to_string`, `measurements_to_int`
- Quantum **Q3-beta**: `histogram` helper
- Quantum **Q3-gamma**: `repetition_encode`, `repetition_decode`, `examples/quantum_bb84/main.sapp`
- Quantum **Q3-delta**: `y_gate`, `z_gate`, `add_depolarizing_noise` in quantum_sim.sapp; `bloch_angles`, `bloch_to_svg` in quantum_viz.sapp; `quantum_noise_depolarizing_pass` test
- Polyglot X1: `--doc` renders polyglot block attrs + code-line counts (text + HTML), doc_cli regression test
- Tooling L1: `init`, `test`, `lint`, `build`, `fmt`, `--doc --html`, `bundle`, `pack`,
  `run`, `--dump-bytecode`; Tooling.md documents stdlib:strings/array/map, dump-bytecode shape
- Tooling L2: unused var/import lint, shadowing lint, error UX file:line:col format,
  sapphire lsp subcommand (JSON-RPC, initialize + textDocument/completion returning builtins)
- Tooling **L3-alpha**: `sapphire fmt` blank line between top-level `fn` declarations
- Tooling **L3-beta**: LSP `open_files` map + scope-aware completion (didOpen/didChange +
  `collect_local_names`), `lsp_completion_includes_local_fn_after_did_open` test
- Tooling **L3-gamma**: LSP `textDocument/hover` + `textDocument/definition` stubs;
  `didOpen` via `serde_json` (correct `\n` handling); tests `lsp_hover_returns_builtin_detail`,
  `lsp_definition_returns_location`; `docs/Tooling.md` L3-gamma section
- Stdlib G1: array.sapp, map.sapp, strings.sapp, math.sapp, graphics.sapp (polygon/bezier/draw_text),
  nd.sapp, db.sapp, ffi.sapp, fs.sapp, http.sapp, json.sapp, net.sapp, ptr.sapp,
  crypto.sapp, os.sapp, path.sapp, time.sapp, csv.sapp (RFC 4180 quoted fields), router.sapp
- Stdlib G2: os.args/os.exit/time.now_iso builtins; process.sapp spawn_wait; csv parse_line (RFC 4180 quoted)
- Stdlib **G3-alpha**: collections.sapp (stack, queue, set)
- Stdlib **G3-beta**: math.sapp scalar helpers (clamp, lerp, smoothstep)
- Threads T3: spawn(fn), join(handle), channel, send, recv, VM-per-thread, transferable captures, multi-channel determinism
- Threads T4: channel_close, channel_is_closed, select(channels_array), thread_pool example
- Threads **T5-alpha**: stdlib/thread.sapp wrapper (spawn, join, channel_new, send_value, recv_value,
  select_ready, channel_close_wrapper, channel_is_closed_wrapper)
- RE R1: --dump-bytecode documented, regression tests
- RE R2: --call-graph (AST-based), --annotate (disassembly + source line headers)
- RE **R3-alpha**: `--dataflow` subcommand
- RE **R3-beta**: `--symbols <file>` symbol table dump (fn + let + bytecode offsets),
  `symbols_lists_function_names` test, Tooling.md R3-beta section
- Spec/Production SP1-alpha: Spec_freeze.md v0.1 (class/match/defer semantics)
- Spec/Production **SP1-beta**: numeric edge cases (Infinity/NaN/f64 limits),
  `numeric_edge_test.sapp`, `numeric_edge_cases_pass`, Spec_freeze.md numeric edge cases section
- Spec/Production **SP1-gamma**: OOP + Match conformance fixes: class constructor dispatch, method resolution with self binding, match wildcard fallthrough and guard evaluation; 4 regression tests passing; Spec_freeze.md v0.2 documented

---

## Pending Tasks

### W4 — Web depth (W4-gamma complete ✓)

All stub files created: stdlib/tls.sapp, tests/semantics/tls_stub_test.sapp, tls_stub_test_pass passing.
Stage B (rustls integration) remains for future work.

> **Next:** Continue priority order from Help.md (A4-gamma complete ✓).

### A4 — AI depth (A4-gamma complete ✓, next: A4-delta)

**A4-gamma** — Chat tool-call example + tensor slice/reshape/dot
- `examples/ai_chat_toolcall/main.sapp`: OpenAI tool-call (function_call) request/response,
  parses `finish_reason == "tool_calls"`, uses `http.post_auth` and `provider_registry`.
- `stdlib/tensor.sapp`: add `slice(t, start, end)`, `reshape(t, new_rows, new_cols)`,
  `dot(a, b)` (matrix product [m×k] × [k×n] → [m×n], pure Sapphire).
- Test: `tests/semantics/tensor_ops_test.sapp` + `tensor_ops_pass` in `tests/semantics.rs`.
- Docs: `docs/Tooling.md` A4-gamma section.

---

### Q3 — Quantum depth (Q3-delta complete ✓)

<!-- Q3-delta completed: y_gate, z_gate, add_depolarizing_noise in quantum_sim.sapp;
     bloch_angles, bloch_to_svg in quantum_viz.sapp; quantum_noise_depolarizing_pass test -->

---

### E5 — Executables (next: E5-gamma)


### L3 — Tooling/LSP (next: L3-delta or L4)

**L3-gamma** — ~~DONE~~ (hover + definition stubs, serde didOpen; see Already Done).

---

### G3 — Stdlib/Graphics (next: G3-gamma)

**G3-gamma** — nd.sapp linear algebra: matrix inverse + solve
- `stdlib/nd.sapp`: add `mat_inv(m)` (2×2 and 3×3 exact inverse via determinant formula,
  pure Sapphire; error map for singular) and `mat_solve(A, b)` (Ax = b for 2×2 and 3×3
  via Cramer's rule).
- Test: `tests/semantics/nd_linalg_test.sapp` + `nd_linalg_inv_solve_pass`.
- Docs: `docs/Tooling.md` G3-gamma section.

---

### M4 — Registry (next: M4-gamma)

**M4-gamma** — Registry token refresh + publish retry
- `crates/sapphire-pm/src/lib.rs`: in `publish_package` for http(s) registries,
  if server returns 401, read `PBLANG_REGISTRY_REFRESH_TOKEN` and POST to
  `<registry>/auth/refresh`; retry publish once with new token.
- `src/main.rs`: `sapphire publish` prints `Refreshed token and retried` on successful retry.
- Test: `tests/install_cli.rs` `publish_retries_on_401_with_refresh_token` (mock via local file registry stub — if 401 scenario too complex to mock, document as design-only and add a unit test for the retry logic in `sapphire-pm`).
- Docs: `docs/Registry_hosted.md` M4-gamma section (token refresh flow).

---

### T5 — Threads (next: T5-beta)

**T5-beta** — Thread pool stdlib helper + deadlock diagnostic
- `stdlib/thread.sapp`: add `thread_pool(n_workers, tasks_array, worker_fn)` —
  spawns n_workers, distributes tasks via internal channel, collects results.
  Uses nil sentinel for graceful shutdown. Results in completion order.
- Test: `tests/semantics/thread_pool_stdlib_test.sapp` + `thread_pool_stdlib_pass`.
- Docs: `docs/Tooling.md` T5-beta section.

---

### R3 — Reverse Engineering (next: R3-gamma)

**R3-gamma** — CFG (control-flow graph) dump
- `src/main.rs`: `sapphire --cfg <file>` — compile to bytecode, walk the `Chunk`
  to identify basic blocks (leaders = first instr, jump targets, instr after jumps),
  emit edges as `BB<n> -> BB<m>` plain text (or DOT format with `--cfg-dot`).
- Test: `tests/cfg.rs` `cfg_output_contains_basic_blocks` — run on
  `tests/fixtures/call_graph_sample.sapp`, assert stdout contains `BB0`.
- Cargo.toml: `[[test]] name = "cfg"`.
- Docs: `docs/Tooling.md` R3-gamma section.

---

### SP1 — Spec/Production (SP1-gamma complete ✓)
**SP1-gamma** — OOP + functional conformance fixes — COMPLETED ✓
All semantics tests now passing (76/76). Spec_freeze.md v0.2 documented.
> **Next:** See priority order (W4, A4, Q3, ...) for upcoming batches.
---

### PL1 — Polyglot (next: PL1-alpha)

**PL1-alpha** — Polyglot block execution bridge
- Allow `polyglot { lang: "python", src: "..." }` blocks to be executed via FFI
  subprocess bridge: spawn Python/Node subprocess, pipe `src`, capture stdout.
- `stdlib/polyglot.sapp`: `run_block(lang, src)` → map{ ok, stdout, stderr, exit_code }.
- Test: `tests/semantics/polyglot_exec_test.sapp` + `polyglot_exec_python_pass`
  (skip if Python not found in PATH).
- Docs: `docs/Tooling.md` PL1-alpha section.

---

### ED1 — Editor (next: ED1-alpha)

**ED1-alpha** — VS Code extension scaffold
- Create `editors/vscode/` with minimal extension:
  `package.json` (name, contributes.languages for `.sapp`/`.sap`, activationEvents),
  `src/extension.ts` stub that starts `sapphire lsp` as a language server via
  `vscode-languageclient`.
- No compilation needed in this task — scaffold only with README install instructions.
- Docs: `docs/Tooling.md` ED1-alpha section.

---

### PF1 — Project/Framework (next: PF1-alpha)

**PF1-alpha** — `sapphire new <template>` scaffold
- `src/main.rs`: `sapphire new <name> [--template web|lib|cli]` — creates `<name>/` dir
  with `pblang.toml` (name, version, entry), `src/main.sapp` stub for chosen template,
  `.gitignore`.
- Templates: `web` (imports http_server, serve_once example), `lib` (export stubs),
  `cli` (os.args usage).
- Test: `tests/new_cmd.rs` `new_creates_web_scaffold` — run `sapphire new myapp --template web`,
  assert `myapp/pblang.toml` and `myapp/src/main.sapp` exist.
- Cargo.toml: `[[test]] name = "new_cmd"`.
- Docs: `docs/Tooling.md` PF1-alpha section.

---

### N2 — Native/OS (next: N2-alpha)

**N2-alpha** — LLVM/Cranelift transpile research spike
- Document concrete steps for Stage C (Native_backend_notes.md):
  define IR → LLVM IR mapping for the 10 most common VM opcodes,
  identify blockers (closures, dynamic dispatch, GC).
- No code change required — docs-only spike unless trivial prototype is possible.
- `docs/Native_backend_notes.md`: add Stage C section (IR mapping table, blockers,
  estimated effort, recommended approach: Cranelift vs LLVM).

---

### OS1 — OS integration (next: OS1-alpha)

**OS1-alpha** — `os.sapp` expansion: env_set, signal handling stub
- `crates/sapphire-vm`: add `os_signal_trap(sig, handler_fn)` builtin stub
  (registers a Sapphire fn to be called on SIGINT/SIGTERM; safe shutdown path).
- `stdlib/os.sapp`: add `signal_trap(sig, fn)` wrapper + `env_set(key, val)` (if not already present).
- Test: `tests/semantics/os_signal_test.sapp` + `os_env_set_pass` (env round-trip only;
  signal test marked `#[ignore]` unless easy to mock).
- Docs: `docs/Tooling.md` OS1-alpha section.

---

## Deferred / Design-only

| Item | Status |
|------|--------|
| TLS full implementation (rustls StreamOwned) | Design note in TLS_notes.md; blocked on StreamOwned lifetime |
| Ed25519 package signing | Design note in Registry_MVP.md; not yet implemented |
| Native single-binary (Stage C) | Research spike only (N2-alpha) |
| Async runtime / green threads | Not planned — VM-per-thread model sufficient for MVP |
| Shared-heap threads | Explicitly excluded — no shared state across VMs |
| `sapphire` REPL | Not prioritized |
Prepared using GPT-5.4 Thinking


Fix 2 failing class semantics tests (constructor dispatch, method resolution)
Fix 2 failing match semantics tests (wildcard fallthrough, guard evaluation)
Add regression tests: class_basic_pass, class_method_dispatch_pass, match_exhaustive_pass, match_guard_pass
Update docs/Spec_freeze.md v0.2 (OOP semantics, closure capture rules, map/filter/reduce)
Update docs/Tooling.md SP1-gamma note
Add thread_pool(n_workers, tasks_array, worker_fn) to stdlib/thread.sapp
Add deadlock diagnostic (detect recv-on-empty with all senders closed)
Create tests/semantics/thread_pool_stdlib_test.sapp
Add thread_pool_stdlib_pass test in tests/semantics.rs
Update docs/Tooling.md T5-beta section
Create examples/ai_chat_toolcall/main.sapp (OpenAI tool-call demo)
Add slice(t, start, end) to stdlib/tensor.sapp
Add reshape(t, new_rows, new_cols) to stdlib/tensor.sapp
Add dot(a, b) matrix product to stdlib/tensor.sapp
Create tests/semantics/tensor_ops_test.sapp
Add tensor_ops_pass test in tests/semantics.rs
Update docs/Tooling.md A4-gamma section
Add SapphireStream trait (wrapping TcpStream or rustls::StreamOwned)
Add tcp_listen_tls(addr, cert_path, key_path) builtin stub
Create stdlib/tls.sapp with listen_tls, serve_once_tls
Create tests/semantics/tls_stub_test.sapp
Update docs/TLS_notes.md status
Add add_depolarizing_noise(circ, p) to stdlib/quantum_sim.sapp
Create stdlib/quantum_viz.sapp (bloch_angles, bloch_to_svg)
Create tests/semantics/quantum_noise_test.sapp
Update docs/Quantum_providers.md + docs/Tooling.md
Implement sapphire bundle --self-extract to produce self-contained binary
Implement extraction logic in crates/sapphire-stub/src/main.rs
Create tests/pack.rs self_extract_binary_contains_sentinel
Update docs/Native_backend_notes.md Stage A status
Handle textDocument/hover in run_lsp_server()
Handle textDocument/definition in run_lsp_server()
Create tests/lsp.rs lsp_hover_returns_builtin_detail + lsp_definition_returns_location
Update docs/Tooling.md L3-gamma section
Add mat_inv(m) to stdlib/nd.sapp (2×2 and 3×3)
Add mat_solve(A, b) to stdlib/nd.sapp (Cramer's rule)
Create tests/semantics/nd_linalg_test.sapp
Add nd_linalg_inv_solve_pass test
Add token refresh logic in crates/sapphire-pm/src/lib.rs publish_package
Handle 401 response with PBLANG_REGISTRY_REFRESH_TOKEN
Create tests/install_cli.rs publish_retries_on_401_with_refresh_token
Update docs/Registry_hosted.md M4-gamma section
Add sapphire --cfg <file> subcommand in src/main.rs
Implement basic block identification and edge emission
Create tests/cfg.rs cfg_output_contains_basic_blocks
Update docs/Tooling.md R3-gamma section
Add run_block(lang, src) to stdlib/polyglot.sapp
Implement FFI subprocess bridge for Python/Node
Create tests/semantics/polyglot_exec_test.sapp
Update docs/Tooling.md PL1-alpha section
Create editors/vscode/ directory with package.json
Create src/extension.ts stub with vscode-languageclient
Add README with install instructions
Update docs/Tooling.md ED1-alpha section
Add sapphire new <name> [--template web|lib|cli] command
Create template directories (web, lib, cli)
Create tests/new_cmd.rs new_creates_web_scaffold
Update docs/Tooling.md PF1-alpha section
Document IR → LLVM IR mapping for top 10 opcodes
Identify blockers (closures, dynamic dispatch, GC)
Update docs/Native_backend_notes.md Stage C section
Add os_signal_trap(sig, handler_fn) builtin stub
Add signal_trap(sig, fn) wrapper to stdlib/os.sapp
Add env_set(key, val) to stdlib/os.sapp (if not present)
Create tests/semantics/os_signal_test.sapp
Update docs/Tooling.md OS1-alpha section
Update docs/Report.md with all completed tasks
Update docs/Tasks.md to reflect current state
Update docs/Help.md with next batch recommendations
Update docs/Tooling.md with all new features


Check current pending tasks in Tasks.md
Identify the 2 failing class semantics tests
Identify the 2 failing match semantics tests
Fix class constructor dispatch (ClassName(args) → fn new)
Fix class method resolution with self binding
Fix match wildcard fallthrough
Fix match guard evaluation after binding
Add regression tests for class basics
Add regression tests for class method dispatch
Add regression tests for match exhaustive
Add regression tests for match guard
Update docs/Spec_freeze.md with v0.2 OOP semantics
Update docs/Spec_freeze.md with closure capture rules
Update docs/Spec_freeze.md with map/filter/reduce contracts
Update docs/Tooling.md with SP1-gamma note


 [ ] Identify failing class semantics tests
- [ ] Identify failing match semantics tests
- [ ] Fix class constructor dispatch (ClassName(args) → fn new)
- [ ] Fix class method resolution with self binding
- [ ] Fix match wildcard fallthrough
- [ ] Fix match guard evaluation after binding
- [ ] Add regression tests for class basics
- [ ] Add regression tests for class method dispatch
- [ ] Add regression tests for match exhaustive
- [ ] Add regression tests for match guard
- [ ] Update docs/Spec_freeze.md with v0.2 OOP semantics
- [ ] Update docs/Spec_freeze.md with closure capture rules
- [ ] Update docs/Spec_freeze.md with map/filter/reduce contracts
- [ ] Update docs/Tooling.md with SP1-gamma note


Read and analyze docs/ folder
Run tests to identify failing class and match semantics tests
Fix 2 failing class semantics tests (constructor dispatch, method resolution)
Fix 2 failing match semantics tests (wildcard fallthrough, guard evaluation)
Add regression tests for class_basic_pass, class_method_dispatch_pass
Add regression tests for match_exhaustive_pass, match_guard_pass
Update docs/Spec_freeze.md v0.2 (OOP semantics, closure capture rules, map/filter/reduce)
Update docs/Tooling.md SP1-gamma note
Update docs/Report.md with completed tasks
Update docs/Tasks.md to reflect current state