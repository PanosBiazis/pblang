# pblang Project Categories

A complete map of every category in the pblang project, organized by layer.

pblang is a **two-tier language**: a frozen native floor written in Rust
(18 crates — the VM kernel plus code-generation backends) and a large
**self-hosted** layer written *in pblang itself* (`.sapp` files — 201 stdlib
modules plus a full compiler-in-pblang under `stdlib/compiler/`). The
"independence from Rust" goal lives entirely at that boundary.

The repository also contains two pblang-written consumer projects that ship
alongside the language: `ai/pbai` (the neural bridge) and
`tools/pblang/pb-mcp-server` (the MCP tool server).

---

## 1. Compiler & Runtime Core (Rust crates — the native floor) — `crates/`

| Category | Crate | Role |
|---|---|---|
| Frontend | `pblang-core` | Parser (chumsky), AST types, polyglot/comment masking |
| IR | `pblang-ir` | OpCode, Chunk, Value, Function, Module; the `.pbck` bytecode container |
| Compiler | `pblang-compiler` | AST → bytecode + optimizer |
| Runtime | `pblang-vm` | Bytecode VM, builtin groups, memory-safety tracker, concurrency, OS signals |
| Types | `pblang-typeck` | Type inference + diagnostics |
| Lint | `pblang-lint` | Lint passes |
| FFI | `pblang-ffi` | Foreign-function interface |

## 2. Code-Generation Backends (compile targets) — `crates/`

- **`pblang-cranelift`** — Cranelift JIT + AOT native object emission
- **`pblang-llvm`** — LLVM IR (`.ll`) text emitter → clang
- **`pblang-ctranspiler`** — pblang → portable C99 (with `pblang_rt.h` runtime header)
- **`pblang-wasm`** — WebAssembly (`.wat` / `.wasm`) backend
- **`pblang-stub`** — self-extracting binary stub for standalone bundles

## 3. Domain Bridges (Rust) — `crates/`

- **`pblang-graphics`** — graphics backend (SDL2 / minifb)
- **`pblang-ai`** — AI/ML provider integration (chat, embeddings, ONNX, tokenization, RAG)
- **`pblang-quantum`** — quantum bridge to Qiskit / Cirq / Braket via Python subprocess

## 4. Developer Tooling (Rust) — `crates/` + `src/`

- **`pblang-lsp`** — Language Server Protocol (tower-lsp): diagnostics, completion, hover, goto-def
- **`pblang-pm`** — package manager: manifest, lockfile, semver, registry, HMAC
- **CLI commands** (`src/`, `src/cli/`):
  `run, build, test, fmt, lint, repl, new, init, check, doc, bench, pack,
  install, publish, add, compile, transpile, bundle, dist, lsp`

## 5. Self-Hosted Compiler (pblang-in-pblang) — `stdlib/compiler/`

tokenizer · parser · semantic · typeck · codegen · optimizer · ir · vm ·
ctranspile · formatter · guard_preprocessor · builtins_list ·
selfhost_frontend · bootstrap

Plus the self-host bundling / output pipeline (`stdlib/`):
`selfhost_bundle.sapp` (linker), `selfhost_to_pbck.sapp` (Rust `.pbck` writer),
`selfhost_pbck.sapp` (self-hosted container: serialize/deserialize),
`selfhost_loader.sapp` (`.pbck` loader: read a file from disk and execute it on
the self-hosted VM — closes the compile→emit→disk→load→run loop), `linker.sapp`,
`dist.sapp`

## 6. Standard Library (201 `.sapp` modules) — `stdlib/`, by domain

- **Web / Net:** http, http_server, http_client, router, middleware, middleware_chain,
  websocket, tls, multipart, net, networking, networking_advanced, url, socks5,
  protocol, web_framework
- **Data / Serialization:** json, csv, dataframe, db, orm, serialization, encoding,
  base32, codec, compression, regex, template
- **Concurrency / Async:** thread, thread_pool, concurrency, async, async_await,
  async_io, parallel, scheduler, pipe, event_bus, observable
- **Math / Science:** math, math_symbols, matrix, tensor, nd, vector, complex,
  fraction, bignum, floatn, stat, science, units, signal, logic
- **AI / ML (in-language):** ai, ml, machine_learning, nn, pbai, pbai_lm, pbai_assets,
  and the `llm_*` family — llm_core, llm_transformer, llm_attention, llm_tokenizer,
  llm_tokenizer_train, llm_train, llm_quant, llm_weights, llm_eval, llm_stream,
  llm_offload, llm_binary, llm_data, llm_adapters
- **Crypto / Security:** crypto, crypto_advanced, crypto_quantum, jwt, tls, security,
  securelayer, safety, memory_safety
- **Graphics / GUI / Game:** graphics, gui, gui_parse, ui_parse, game, ecs, color,
  colors, bitmap_font, image_processing, img_load, browser, browser_render,
  design_engine, visual_engine, html, syntax_hl
- **Quantum:** quantum, quantum_sim, quantum_viz, quantum_protocol
- **System / OS:** os, fs, path, process, env, hwinfo, systems, time, date, datetime,
  duration, log, logger, profiler, benchmark
- **Polyglot / Interop:** polyglot, polyglot_build, polyglot_db, polyglot_web,
  polyglot_registry, interop, ffi, ffi_universal, ptr, build, pkg, dist
- **Language core / utilities:** array, map, set, deque, heap, graph, collections,
  data_structures, iter, functional, generics, typeclass, enum, struct, class,
  option, result, validate, format, strings, unicode, uuid, money, semver
- **Observability / Control:** watch, perception, master_control, dashboard,
  debugger, event, state_machine
- **MCP / Agent:** pb_mcp, pbp, pbz, registry, registry_server, provider_registry,
  resources

## 7. OS-Dev Kernel (bare-metal pblang) — `stdlib/kernel/`

console · gc · gdt · heap · interrupts · keyboard · paging · sched · syscall · timer

## 8. Tests — `tests/`

- **`tests/semantics/`** — 225 `.sapp` language / semantics tests
- **Rust integration tests (~35):** typeck_compiler, lint, fmt, fmt_drift, lsp, modules,
  pack, pbck_cli, quantum_simulator, sandbox, selfhost_drift, tensor_builtins,
  neural_net_builtins, signal_builtins, encoding_builtins, threads, http_server,
  golden, install_cli, new_cmd, doc_cli, dataflow, cfg, call_graph, block_expr,
  if_else, annotate, dump_bytecode, lockfile_repro, stdlib_parse, string_limits,
  symbols, ctranspiler_runtime

## 9. Examples & Tutorials — `examples/`

- **Tutorials:** cli, concurrency, data_science, http, machine_learning,
  memory_safety, user_input, web_app
- **Demos:** hello_world, ai_chat_toolcall, ai_http_provider, ai_plugin_minimal,
  bend_style_parallel, concurrent_tasks, thread_pool, orm_example, polyglot_demo,
  polyglot_full, quantum_bb84, quantum_bridge_minimal, self_host_demo, web_ide,
  web_server

## 10. Editor / IDE Integration — `editors/`

- VS Code extension — `editors/vscode/` (syntaxes + src)
- Standalone IDE — `editors/ide/`

## 11. Sibling pblang projects (written *in* pblang)

- **`ai/pbai/`** — the Ideal-NN ↔ real LM bridge (`stdlib/pbai_ideal_bridge.sapp`),
  plus server, demo, website, checkpoints
- **`tools/pblang/pb-mcp-server/`** — MCP server with 40+ tool modules behind
  `router.sapp`: agent, ai, algorithm, app, chart, codebase, code_index, completion,
  consensus, context, convert, core, debug, file, git, history, ide, log, mcp, media,
  ml, net_iot, project, protect, quantum, re, review, runtime, sandbox, security,
  shell, system, terminal, text, thinking, wait, watcher
