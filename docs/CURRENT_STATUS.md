# pblang — Current Status

**Last Updated**: 2026-05-30  
**Status**: v0.5.7 — Production-Ready Core with AI Integration, Security Hardening + Concurrency + Memory Safety + 135 Stdlib Modules + **Complete Self-Hosting Compiler (tokenize→parse→compile→RUN, all in pblang)** + Multi-Backend Codegen
**Test Suite**: semantics 189 passed / 0 failed / 1 ignored (only `os_env_set_pass`, an `env_set` safety policy). The former match-guard parser limitation is **resolved** — `match_guard_pass` and the `:`-separator variants now pass.

---

Canonical status is the top summary and `Test Results` section in this file. Older milestone snapshots below are kept for historical context and may reflect prior counts/plans.

## ⭐ 2026-05-30 — Self-hosting is COMPLETE (the headline)

The pblang-written compiler under `stdlib/compiler/` now **tokenizes, parses, compiles, and RUNS** pblang end-to-end, entirely in pblang. `selfhost_frontend.frontend_run(source)` drives source → tokenize → parse → typecheck → codegen → `vm.sapp` bytecode interpreter and returns `{output, locals, globals}`. **Every core language feature self-hosts**: let/const, arithmetic/comparison, logical short-circuit, arrays/maps, `if`/`else if`/inline-`if` expressions, `while`/`for`/`for_in`, `break`/`continue`, functions (recursion + default params), index/member read+assign, `try`/`catch`, ~30 host builtins, module globals, nil-safe `??`/`?.`/`?[`, `match`, `struct`/`enum`, `fn_lit` (closures), member/index callees (`obj.f(args)`, `tbl[i]()`), and `class` + colon-call `obj:method(args)`. **No language gaps remain**; the last consistency item (`strcharcodeat` byte-vs-codepoint alignment) is closed — the self-host frontier is now breadth/perf. Locked by `tests/selfhost_drift.rs` (26 active + 1 ignored perf case) and `tests/typeck_compiler.rs` (5/0). See `docs/PrjAnalysis.md` §§40–45.

## 🤖 v0.5.6 AI Integration

| Feature | Description |
|---------|-------------|
| `pblang-ai` crate | Full AI/ML integration library with provider registry, chat completion, embeddings, token counting, RAG pipelines |
| `ai_chat()` builtin | Chat completion via OpenAI, Anthropic, Ollama, or custom providers — returns JSON response |
| `ai_embed()` builtin | Text embedding generation via any registered provider — returns JSON with vector + dimensions |
| `ai_token_count()` builtin | Token estimation for prompt budgeting (chars/1.3 and chars/4 heuristics) |
| `onnx_infer()` builtin | Updated with clearer error message directing users to ai_chat/ai_embed or FFI for ONNX models |
| Provider auto-registration | Auto-detects `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `OLLAMA_URL` env vars |
| Streaming support | `chat_stream()` in the AI crate for SSE-based streaming responses |
| Vector math | `cosine_similarity()`, `dot_product()`, `euclidean_distance()` in the AI crate |
| RAG helpers | `chunk_text()`, `semantic_search()` for retrieval-augmented generation pipelines |
| Sandbox integration | All AI builtins respect `allow_http`/`allow_net` capability flags |

---

## 🔒 v0.5.2 Concurrency Module Improvements

| Primitive | Description | Pattern |
|-----------|-------------|---------|
| `SafeMutex` | Production-quality mutual exclusion with timeout and RAII `with_mutex()` | Rust `Arc<Mutex<T>>`, Go `sync.Mutex` |
| `SafeRwLock` | Read-write lock allowing concurrent readers or exclusive writer | Rust `RwLock<T>`, Go `sync.RWMutex` |
| `SafeChannel` | Bounded/unbounded channels with `send`/`recv`/`try_send`/`try_recv` and timeout | Go `chan T`, Rust `mpsc` |
| `ThreadPool` | Fan-out/fan-in worker pool: `thread_pool(n_workers, tasks, fn)` | Java `ExecutorService`, Go worker pools |
| `Barrier` | Cyclic barrier for N-thread rendezvous (`barrier_new`/`barrier_wait`) | Rust `Barrier`, Java `CyclicBarrier` |
| `Once` | One-time initialization guarantee (`once_new`/`once_do_lazy`) | Go `sync.Once`, Rust `Once` |
| `Timer` | One-shot timers with `wait`/`cancel`/`remaining_ms` | Go `time.Timer`, Rust timeouts |
| `WaitGroup` | Coordinated goroutine completion (`wait_group`/`wg_done`/`wg_wait`) | Go `sync.WaitGroup` |
| `Semaphore` | Counting semaphore for rate limiting (`semaphore`/`acquire`/`release`) | Go `semaphore.Weighted`, Java `Semaphore` |
| `CircuitBreaker` | Resilience pattern: CLOSED → OPEN → HALF-OPEN transitions | Netflix Hystrix, resilience4j |
| `RateLimiter` | Token-bucket rate limiter with auto-refill ticker | Go `rate.Limiter` |
| `FanOut`/`FanIn` | Channel distribution (round-robin fan-out) and merging (fan-in) | Go pipeline patterns |

## 🛡️ v0.5.7 Runtime Memory Safety (VM builtins)

| Feature | Description |
|---------|-------------|
| `MemoryTracker` in VM | Per-VM ownership/borrow/lifetime tracker in `crates/pblang-vm/src/memory_safety.rs` |
| **VM builtins wired** | `allocate`, `borrow_immutable`, `borrow_mutable`, `borrow_unique`, `release_borrow`, `load`, `store`, `deallocate`, `push_lifetime`, `pop_lifetime`, `memory_stats` |
| Borrow rules | Mutable borrows exclusive; multiple immutable borrows allowed; `store` rejects active borrows; `borrow_unique` = use-after-move |
| Lifetime scoping | `push_lifetime` / `pop_lifetime` invalidate values allocated in popped scopes |
| Tests | `tests/semantics/memory_safety_builtins_test.sapp` + Rust unit tests on `MemoryTracker` |
| **Not yet** | Compile-time borrow checking for ordinary values; `stdlib/memory_safety.sapp` remains a sketch (use builtins instead) |

## 🔒 v0.5.1 Security Fixes (Comprehensive Audit)

| Issue | Severity | Fix |
|-------|----------|-----|
| `DangerousAllowAllVerifier` — TLS accepted ANY cert (MITM) | 🔴 Critical | Replaced with webpki-roots + rustls-native-certs |
| Path traversal in `static_file_response()` | 🟠 High | `_path_safe()` strips `../`, URL-decodes, returns 403 |
| Command injection in `os.which()` | 🟡 Medium | New `os_which` builtin searches PATH safely without shell |
| `fs.sapp` zero validation | 🟠 High | Absolute paths + `../` blocked by default; opt-in `*_abs()` |
| URL query params not decoded | 🟡 Medium | `url_decode()` added to `_parse_query()` |
| `NonNull` (T!) type erased | 🟡 Medium | `Ty::NonNull` preserved; nil→T! emits diagnostic |
| Only 4 HTTP status codes | 🟡 Medium | Full RFC 7231 table (60+ codes) via `http_status_reason()` |
| `env_get` required exec cap | 🟡 Low | Reading env vars is always safe; cap check removed |
| Missing builtins: `str_replace_all`, `hmac_sha256` | 🟡 Medium | Both implemented |
| MIME types missing charset | 🟡 Low | 30+ types with proper charset annotations |


## 🚀 v0.5.6 Bit-Level Optimizations & New Modules

| Feature | Description |
|---------|-------------|
| String Constant Folding | Compile-time string concatenation folding in optimizer (`"a" + "b"` → `"ab"`) |
| Peephole Optimizer | Redundant op removal: `Dup+Pop`, `Not+Not` → skip |
| Jump Threading | Multi-pass jump-to-jump resolution for branch optimization |
| `bitops.sapp` | 26 bit-level operations: AND/OR/XOR/NOT/SHL/SHR, popcount, CLZ/CTZ, bit test/set/clear/toggle, rotations, byte swap, binary/hex conversion |
| `compression.sapp` | Gzip, Zstd, Brotli compress/decompress + auto-detect |
| `serialization.sapp` | JSON pretty/validate, TOML/YAML parse/stringify, CSV parse/stringify, XML escape/unescape |


## 🚀 v0.5.6 — Generic Types + 6 New Stdlib Modules + Full Algorithm Suite

| Feature | Description |
|---------|-------------|
| Generic Type Resolution | `TypeEnv` with generic binding/substitution, `is_subtype_of()` variance rules, `TraitDef` + `TraitRegistry` |
| `machine_learning.sapp` | 10 functions: normalize, train_test_split, accuracy, confusion_matrix, kfold, one_hot, euclidean/manhattan distance |
| `blockchain.sapp` | 10 functions: sha256d, merkle_root, mine_block, verify_block, create_wallet, sign/verify, transactions |
| `networking_advanced.sapp` | 12 functions: ping, dns_lookup, whois, port_scan, ssl_expiry, http_download, url parse/encode/decode |
| `data_structures.sapp` | 28 functions: Stack, Queue, PriorityQueue, LinkedList, Trie, LRU Cache, DisjointSet (Union-Find) |
| `image_processing.sapp` | 10 functions: grayscale, ASCII art, bilinear resize, box blur, Sobel edges, threshold, invert, brightness/contrast |
| `algorithms_advanced.sapp` | 10 functions: binary_search, quick_sort, merge_sort, levenshtein, LCS, BFS, DFS, fibonacci_dp, knapsack, dijkstra |


## 🚀 v0.5.6 — LSP Goto-Def + Self-Hosting Type Checker + OS Groundwork

| Feature | Description |
|---------|-------------|
| **LSP Goto-Definition** | `textDocument/definition` — jump to fn/let/const definitions |
| **LSP Hover** | 40 builtin signatures + local fn/let type info |
| **LSP Completions** | Builtins + keywords + local declarations |
| Self-Hosting Type Checker | `compiler/typeck.sapp` — completes the 4-stage pipeline: **tokenizer → parser → type_checker → codegen** |
| Native Bitwise Opcodes | `OpBitAnd/Or/Xor/Not/Shl/Shr` in VM for 10-50x speedup |
| `systems.sapp` Enhanced | OS groundwork: inline asm stubs, port I/O, MMIO, raw pointers, interrupts, page tables, GDT, multiboot, hardware detection |

## ✅ What Works (Production-Ready)

| Component | Status | Notes |
|-----------|--------|-------|
| Core VM | ✅ Stable | Bytecode, GC, sandboxing, error handling |
| Package Manager | ✅ Functional | Lockfile, semver, file/hosted registry, HMAC verify |
| Stdlib | ✅ 135 modules | HTTP, WebSocket, SQLite, crypto, threads, quantum, AI, tensor |
| Threading | ✅ Working | VM-per-thread, channels, select, thread pool |
| OOP/Types | ✅ Working | class, struct, enum modules with full test coverage |
| New Stdlib | ✅ Added | testing, orm, async, env, log, jwt, template, graph, benchmark, crypto_advanced, class, struct, enum, http_client |
| CLI Tooling | ✅ Complete | run, test, fmt, lint, build, install, publish, search |
| Type Checker | ✅ Improved | Builtin prelude, generic call substitution, fn-literal body checks; the self-hosted `.sapp` compiler now parses/compiles/runs the full language (parity reached) |
| LSP Server | ✅ Unified | `pblang lsp` → `pblang-lsp` (tower-lsp): diagnostics, completions, hover, goto-def, workspace symbols |
| Native Executables | ✅ Production | `pblang compile` (C99), `bundle`, `pack`, `bundle --self-extract` (embedded runtime at `bin/pblang`) |
| TLS/HTTPS | ✅ Stable | rustls + webpki-roots, proper certificate verification |
| Concurrency | ✅ Enhanced | SafeMutex, SafeRwLock, SafeChannel, ThreadPool, Barrier, Once, Timer |
| Memory Safety | ✅ Wired | Runtime builtins + `MemoryTracker`; not compile-time for all values |
| Async/Await | ✅ Added | `async_spawn`, `await`, `await_all`, `await_any`, `map`, `then` via `stdlib/async_await.sapp` |
| Quantum Computing | ✅ Bridge | `pblang-quantum` crate: Qiskit, Cirq, Braket via Python subprocess; `quantum_run`, `quantum_statevector`, `quantum_backends` builtins |
| Polyglot Execution | ✅ Bridge | `polyglot_exec(lang, code)` builtin: execute Python, JS, Bash, Ruby, Perl, Lua, PHP, R inline |
| WASM Backend | ✅ Prototype | `pblang-wasm` crate: compile pblang AST → WAT (.wat); functions, arithmetic, strings, exports |
| LLVM Backend | ✅ Prototype | `pblang-llvm` crate: LLVM IR text emitter → compile with clang; `pblang compile --llvm`; functions, if/else, while, for, comparisons |
| Self-Hosting | ✅ **Complete** | `stdlib/compiler/` (tokenizer→parser→semantic→typeck→ir→optimizer→codegen + `selfhost_frontend.sapp` entry + `vm.sapp` interpreter, ~5,300 lines) tokenizes, parses, compiles **and runs** pblang end-to-end. All language features self-host (incl. `match`, `struct`/`enum`, `class`+colon-call, closures, nil-safe ops). Frontier is now breadth/perf |
| Polyglot Build | ✅ Bridge | `pblang build-polyglot`: extracts @python/@rust/@js blocks from .sapp files, compiles to .so/.pyc, generates manifest |
| Math Library | ✅ Enhanced | `math.sapp`: trig (sin/cos/tan), hyperbolic, stats (mean/median/stddev/variance), linear algebra (dot/mat_mul/vec_norm), constants (PI/E/TAU/PHI), number theory (gcd/lcm/is_prime) |
| New Stdlib | ✅ Added | `units.sapp` (150+ conversions), `functional.sapp` (map/filter/reduce/compose/pipe/curry/memoize), `encoding.sapp` (hex/URL/CSV/base64/JSON), `colors.sapp` (HEX/RGB/HSL/blending/palettes) |
| Type Checker | ✅ Enhanced | "did you mean?" suggestions for undeclared variables (Levenshtein distance), better error messages |

## ✅ Test Results

```
cargo test --test semantics      → 189 passed; 0 failed; 1 ignored
cargo test --test selfhost_drift → 26 passed; 0 failed; 1 ignored
cargo test --test modules        → 28 passed; 0 failed
cargo test --test typeck_compiler→ 5 passed; 0 failed
cargo test -p pblang-typeck      → 46 passed; 0 failed
cargo test --test fmt            → 8 passed; 0 failed
cargo test --test fmt_drift      → 2 passed; 0 failed
cargo test -p pblang-vm --lib    → 2 passed; 0 failed
```

Ignored tests are currently:
- **semantics:** `os_env_set_pass` — kept ignored because `env_set()` stays disabled in v0.5 security hardening (`std::env::set_var` is unsound in multi-threaded programs). This is the *only* ignored semantics test.
- **selfhost_drift:** one perf-gated case — running the full `stdlib/compiler/parser.sapp` *on the self-hosted interpreter* takes >60s per snippet; un-ignore once the self-hosted parser is fast enough to default-on.

> The former match-guard limitation (arm guards + `:` separator vs. the `obj:method()` colon-call postfix) is **resolved**: `match_guard_pass`, `match_guard_accepts_colon_separator`, and `match_guard_still_accepts_tilde_separator` all pass. Guards work with `:`, `then`, and `#` separators.

## 🚀 Key Improvements Since v0.4

1. **Concurrency module** — Production-quality `SafeMutex`, `SafeRwLock`, `SafeChannel`, `ThreadPool`, `Barrier`, `Once`, `Timer`, `WaitGroup`, `Semaphore`, `CircuitBreaker`, `RateLimiter`, `FanOut`/`FanIn` in `stdlib/thread.sapp` and `stdlib/concurrency.sapp`
2. **Memory safety module** — `MemoryTracker` + VM builtins (`allocate`, `borrow_*`, `load`, `store`, lifetimes); semantics test `memory_safety_builtins_pass`
3. **Async/await primitives** — `stdlib/async_await.sapp` providing `async_spawn`, `await`, `await_all`, `await_any`, `map`, `then` built on channel/thread infrastructure
4. **Spawned thread builtins** — Child VMs now inherit the parent VM's builtins module
   via parent chain, so `recv`, `send`, `channel`, etc. work inside spawned closures
5. **`select()` now returns `{index, value}`** — Previously returned just index number
6. **channel_close/is_closed** — Now correctly accept sender handle numbers
7. **String char-code builtins**: `strfromcharcode`, `strcharcodeat` — both **codepoint-based** (as of 2026-05-30): `strcharcodeat(s, i)` returns the full Unicode scalar at codepoint index `i`, consistent with `len`/`s[i]`/`slice`/`str_index_of`, so `strfromcharcode(strcharcodeat(s,i))` round-trips. For raw bytes use `str_to_bytes(s)` → `Array<u8>`
8. **stdlib/websocket.sapp** — Full `write_frame(opcode, payload)` implementation
9. **stdlib/http.sapp** — Added `parse_sse_line`, `post_auth`
10. **stdlib/quantum.sapp** — Added `to_qasm`, `measurements_to_string`, `measurements_to_int`,
   `apply_h`, `apply_cnot`, `measure_all`, `n_qubits` key on circuit objects
11. **stdlib/provider_registry.sapp** — Added `call_best`, `call_with_fallback`
12. **stdlib/collections.sapp** — Added `set_union`, `set_intersect`; queue is now array-based
13. **stdlib/math.sapp** — Added `util.clamp`, `util.lerp`, `util.smoothstep`
14. **stdlib/http_server.sapp** — Added `sse_event`, `res_sse_headers`
15. **stdlib/thread.sapp** — Fixed `channel_close_wrapper`/`channel_is_closed_wrapper`
16. **spawn capture reliability** — `spawn(...)` now snapshots open upvalues from the parent VM stack at spawn time, so common channel-capture closures work without unsafe transfer semantics
17. **upvalue crash hardening** — stale/open upvalue access now uses bounds-safe fallback behavior (`nil`) instead of out-of-bounds panic in close/get/set paths
18. **spawn/join closure wire format** — thread handoff now supports transferring function/closure values safely (when captures are transferable), unblocking parallel worker closures in `thread_pool`

## 🔴 Critical Gaps (Production Blockers)

| Issue | Priority | Effort |
|-------|----------|--------|
| ~~No TLS/HTTPS~~ | **✅ DONE** | Implemented with rustls |
| ~~Native executables incomplete~~ | **✅ DONE** | `bundle --self-extract` ships on Linux (Windows CI follow-up) |
| ~~Match guard body separator ambiguity~~ | **✅ RESOLVED** | Guards work with `:`/`then`/`#`; all match-guard tests pass |
| ~~Type system stub~~ | **✅ N/A** | Real prelude + generic substitution + fn-literal checks; struct/enum/class supported |
| Self-hosted parser perf | LOW | Speed up `parser.sapp` on the interpreter to drop the `selfhost_drift` `#[ignore]` |

## 📋 Recommended Next Steps

The self-host frontier is now **breadth/perf**, not grammar (all language features self-host):

1. **Differential run** — run a real `stdlib/*.sapp` module through `frontend_run` and diff output against the Rust VM.
2. **Self-hosted parser speedup** — get `parser.sapp` fast enough on the interpreter to drop the `#[ignore]` on the full-parser `selfhost_drift` case.
3. **Bootstrap probe** — feed `frontend_compile`'s own source through `frontend_run` and measure how far the pblang compiler gets toward compiling itself.
4. **Cranelift JIT** (N2-beta) — native code compilation for hot paths.
5. **LSP depth** — semantic tokens, richer type-aware hover, cross-file intelligence.

**Longer-term vision:** **pbAI** — a native AI model family (a flagship pbAI-Max plus
distilled, efficient sub-models) **written in pblang**, used as a coding/debugging agent
to make pblang better, driving the **pb-mcp-server** tool catalog. Design: `docs/pbAI.md`.

## Archived snapshot (legacy notes below)

The sections below are preserved for historical traceability and may include
older milestone wording or counts. Use the top summary and `Test Results`
section as canonical current status.

---

## 📊 Executive Summary

pblang has evolved from a promising prototype into a robust, production-quality core runtime with an expanding ecosystem. The language successfully balances two distinct use cases:

1. **Embedded Scripting**: Secure, sandboxed scripting for applications/games/tools
2. **Standalone General-Purpose**: Full-featured programming language with package ecosystem

### Maturity Assessment

| Component | Status | Grade | Notes |
|-----------|--------|-------|-------|
| Core VM | ✅ Stable | ⭐⭐⭐⭐⭐ | Bytecode execution, GC, sandboxing production-ready |
| Package Manager | ✅ Functional | ⭐⭐⭐⭐ | Local & remote registry working, lockfile v2 ready |
| Stdlib | ✅ Comprehensive | ⭐⭐⭐⭐ | 20+ modules covering web, AI, quantum, crypto |
| Semantics Tests | ✅ Passing | ⭐⭐⭐⭐⭐ | 147/149 tests passing (2 ignored) |
| Thread Safety | ✅ Working | ⭐⭐⭐⭐ | VM-per-thread model with channels |
| Native Executables | ✅ Self-extract | ⭐⭐⭐ | `bundle --self-extract` embeds runtime; E2E on Linux |
| LSP Server | ✅ Unified | ⭐⭐⭐ | tower-lsp via `pblang lsp`; diagnostics, completion, hover, goto-def, workspace symbols |
| Type System | ⚠️ Stub | ⭐ | Crate exists but unimplemented |
| TLS/HTTPS | ✅ Stable | ⭐⭐⭐⭐ | rustls wired (`tcp_listen_tls`, `tcp_connect_tls`, `tcp_wrap_tls`) |

---

## ✅ What's Already Working (Production-Ready)

### 1. Core Language Runtime ⭐⭐⭐⭐⭐

**Status**: PRODUCTION-READY

The VM core is stable and battle-tested:

- **Bytecode Execution**: Efficient stack-based VM with compiled bytecode
- **Memory Management**: RC + cycle-breaking GC (adequate for most workloads)
- **Capability-Based Security**: 
  - `VmLimits` (max_instructions, max_stack_depth, io_timeout_ms)
  - `FfiPolicy` with allowlist
  - `StdlibConfig` for feature gating
- **Error Handling**: Comprehensive `RuntimeError` system with spans and stack traces
- **Module System**: 
  - Local imports: `import "./lib.sapp"`
  - Package imports: `import "stdlib:thread.sapp"`
  - Export system with `export` keyword
- **Testing**: 147/149 semantics tests passing (2 ignored)

**Files**:
- `crates/pblang-vm/src/vm.rs` - Core VM (stable)
- `crates/pblang-core/src/parser.rs` - Parser (stable)
- `crates/pblang-compiler/src/lib.rs` - Compiler (stable)

### 2. Package Manager ⭐⭐⭐⭐

**Status**: FUNCTIONAL (registry polish + lockfile v2)

- **Lockfile v2**: `BTreeMap` deps, `archive_sha256`, `fetch_url`, git `rev`; content-hash verify on install/run
- **Registry resolve**: relative tarball URLs, post-unpack validation (always, including cache hits), symlink rejection, HMAC sidecar
- **Rehydrate**: registry/git deps from lock when cache path missing (`fetch_url` or `PBLANG_REGISTRY_INDEX`)
- **Imports**: `import("@scope/pkg")`, `import("pkg:subpath")`, bare git package names via lockfile
- **Publish**: module/entry validation, immutable version republish; `publish --registry-dir` for local registries
- **Search**: `file://` registry dirs read `index.json` locally (hosted HTTP search unchanged)
- **Tests**: `tests/install_cli.rs` (31 cases), `cargo test -p pblang-pm`

**Files**:
- `crates/pblang-pm/src/lib.rs` - Package manager core
- `tests/install_cli.rs` - Registry CLI integration tests
- `docs/Language_Semantics.md` - Package import section
- `docs/Registry_MVP.md`, `docs/Registry_hosted.md`

### 3. Standard Library ⭐⭐⭐⭐

**Status**: COMPREHENSIVE (20+ Modules)

| Module | Status | Features |
|--------|--------|----------|
| `thread.sapp` | ✅ Enhanced | spawn, join, channels, select, WaitGroup, Barrier, Once, Semaphore, CircuitBreaker, RateLimiter, FanOut/FanIn |
| `concurrency.sapp` | ✅ Added | SafeMutex, SafeRwLock, SafeChannel, ThreadPool, Barrier, Once, Timer, Atomic, concurrency stats |
| `memory_safety.sapp` | 🟡 Sketch | Real API is VM builtins; see `Language_Semantics.md` |
| `async_await.sapp` | ✅ Added | async_spawn, await, await_all, await_any, map, then |
| `http.sapp` | ✅ Stable | GET, POST, streaming, SSE |
| `http_server.sapp` | ✅ Stable | TCP server, routing, middleware, timeouts |
| `json.sapp` | ✅ Stable | parse, stringify |
| `crypto.sapp` | ✅ Stable | SHA256, base64, random bytes |
| `db.sapp` | ✅ Stable | SQLite wrapper, migrations |
| `tensor.sapp` | ✅ Working | Basic tensor operations |
| `quantum.sapp` | ✅ Working | Circuit builder, QASM emitter |
| `quantum_sim.sapp` | ✅ Working | Measurement, histogram, ECC helpers |
| `collections.sapp` | ✅ Stable | Stack, queue, set helpers |
| `math.sapp` | ✅ Stable | Complex, lerp, clamp, smoothstep |
| `csv.sapp` | ✅ Stable | CSV parsing with quoting |
| `websocket.sapp` | ✅ Working | Frame parsing and writing |
| `graphics.sapp` | 🟡 MVP | Basic graphics primitives |
| `ffi.sapp` | ✅ Stable | C ABI bindings |
| `fs.sapp` | ✅ Stable | File I/O |
| `net.sapp` | ✅ Stable | TCP sockets |
| `os.sapp` | ✅ Stable | Process, environment |
| `path.sapp` | ✅ Stable | Path manipulation |
| `time.sapp` | ✅ Stable | Time utilities |

**Files**:
- `stdlib/*.sapp` - All stdlib modules
- `tests/semantics/*.sapp` - Comprehensive tests for each module

### 4. Threading & Concurrency ⭐⭐⭐⭐

**Status**: MVP COMPLETE (per-VM handle registry)

- **VM-per-Thread Model**: Each thread gets isolated VM instance; spawn/join/channel/sync handles live in `VmHandleRegistry` (`Arc` shared with child VMs from `spawn`, not process-global)
- **Channels**: Message passing with `channel()`, `send()`, `recv()`
- **Select**: Multi-channel select with `select(channels_array)`
- **Stdlib Wrapper**: `stdlib:thread.sapp` provides ergonomic API
- **Thread Pool Helper**: ✅ Implemented (T5-alpha)

**Example**:
```sapphire
import "stdlib:thread.sapp";

let jobs = thread.channel_new();
let results = thread.channel_new();

let worker = thread.spawn(fn() {
    let job = thread.recv_value(jobs.receiver);
    while job != nil {
        thread.send_value(results.sender, job * 2);
        job = thread.recv_value(jobs.receiver);
    }
});

thread.send_value(jobs.sender, 5);
thread.send_value(jobs.sender, nil); // termination signal
thread.join(worker);
```

**Files**:
- `stdlib/thread.sapp` - Thread stdlib wrapper
- `tests/semantics/thread_stdlib_test.sapp` - Thread test suite

### 5. CLI Tooling ⭐⭐⭐⭐

**Status**: COMPREHENSIVE

All major commands implemented:

| Command | Status | Description |
|---------|--------|-------------|
| `sapphire run` | ✅ | Execute .sap/.sapp files |
| `sapphire check` | ✅ | Parse + typecheck without execution |
| `sapphire test` | ✅ | Run test suite |
| `pblang install` | ✅ | Install dependencies or packages |
| `pblang publish` | ✅ | Publish to registry |
| `pblang search` | ✅ | Search packages |
| `pblang bundle` | 🟡 | Package app (self-extract pending) |
| `sapphire doc` | ✅ | Generate documentation |
| `sapphire fmt` | ✅ | Format source code |
| `sapphire lint` | ✅ | Lint source code |
| `sapphire --annotate` | ✅ | Annotated disassembly |
| `sapphire --symbols` | ✅ | Symbol table dump |
| `sapphire --dump-bytecode` | ✅ | Bytecode disassembly |

**Files**:
- `src/main.rs` - CLI implementation
- `docs/Tooling.md` - CLI reference

### 6. Semantics Test Suite ⭐⭐⭐⭐⭐

**Status**: 100% PASSING (143/143 tests)

**Test Categories**:
- ✅ Block expressions and shadowing
- ✅ Closures and captures
- ✅ Arrays and maps
- ✅ JSON parsing/stringification
- ✅ Thread stdlib wrapper
- ✅ HTTP server with timeouts
- ✅ Quantum circuits and measurement
- ✅ Tensor operations
- ✅ AI streaming inference
- ✅ Collections (stack/queue/set)
- ✅ Math extensions
- ✅ WebSocket frames
- ✅ CSV parsing

**Known Issue**:
- `thread_stdlib_wrapper_pass` may timeout on slower machines (not a failure, just needs timeout protection)

**Files**:
- `tests/semantics.rs` - Test harness
- `tests/semantics/*.sapp` - Individual test scripts

---

## 🟡 In Progress / Needs Completion

### 1. Native Executables (E5) - Self-extract complete on Linux

**What's Done**:
- ✅ `crates/pblang-stub` — extract, run, `--extract-only`, `--bundle-version`, temp cleanup
- ✅ `pblang bundle --self-extract` — stub + sentinel + tarball with embedded `bin/pblang`
- ✅ E2E tests: sentinel, run without `PATH`, `--extract-only` includes runtime
- ✅ Portable `pblang bundle` (folder with copied `pblang` binary)

**Remaining** (follow-up):
- Windows CI for self-extract E2E
- Optional: true single-file without temp extract (native compile backend)

### 2. TLS/HTTPS Support (W4) - Implemented

**Priority**: MEDIUM  
**Blocker**: none (shipping; follow-up hardening only)

**What's Done**:
- ✅ Design in `docs/TLS_notes.md`
- ✅ Per-request timeout in HTTP server (W4-alpha)
- ✅ SSE support in HTTP server
- ✅ `tcp_set_timeout()` builtin

**What's Missing**:
- ⏳ Expanded cross-platform cert-path test matrix
- ⏳ More end-to-end TLS docs/examples for deployment presets

**Required Work** (follow-up):
1. Keep certificate validation regression tests in CI
2. Expand deployment playbooks (local certs, hosted certs, proxies)
3. Document operational hardening defaults

### 3. Type System - 10% Complete

**Priority**: LOW (for v1.0)  
**Blocker**: Major implementation effort

**What's Done**:
- ✅ `crates/pblang-typeck` crate created
- ✅ Type annotation syntax in parser

**What's Missing**:
- ❌ Type inference engine
- ❌ Type checking pass
- ❌ Struct/enum syntax
- ❌ Generic types
- ❌ Type error reporting

**Status**: Stub only, requires significant design work

---

## ⚠️ Critical Gaps (Production Blockers)

### 1. ~~No TLS/HTTPS~~ - ✅ COMPLETE

**Status**: Implemented in v0.5 with rustls
- `tcp_listen_tls(addr, cert_path, key_path)` — TLS server listener
- `tcp_connect_tls(host, port)` — TLS client connection
- `tcp_wrap_tls(handle, cert_path, key_path)` — upgrade plain TCP to TLS
- Client verification uses trust roots (`webpki-roots` + native roots where available)

### 2. Native Executables Incomplete - 🔴 HIGH (now #1 priority)

**Impact**: Cannot distribute single-binary applications  
**Workaround**: Distribute source + `pblang` runtime  
**Effort**: 1-2 weeks

### 3. No Type System - 🟢 MEDIUM

**Impact**: Limited IDE support, no compile-time type safety  
**Workaround**: Use `pblang --check` for syntax/type diagnostics  
**Effort**: 2-3 months

---

## 🎯 Recommended Next Steps (Next 2-4 Weeks)

### Week 1-2: Native Executables (E5) - HIGH PRIORITY

**Goal**: Self-extracting single-binary executables

**Tasks**:
1. Implement tarball extraction in `pblang-stub`
2. Add binary concatenation logic to `pblang bundle --self-extract`
3. Test Linux executable generation
4. Test Windows executable generation
5. Add cleanup handlers for temp directories

**Deliverable**: `pblang bundle --self-extract --output myapp` produces working `myapp` executable

### Week 3-4: TLS/HTTPS Support (W4) - HIGH PRIORITY

**Goal**: Production-ready HTTPS servers

**Tasks**:
1. Add `rustls` dependency
2. Implement `SapphireStream` trait
3. Add `tcp_listen_tls()` builtin
4. Modify `tcp_accept` for TLS transparency
5. Create `stdlib/tls.sapp` wrapper
6. Test HTTPS server with self-signed cert

**Deliverable**: HTTPS server example works with `openssl s_client`

### Week 5+: Ecosystem Expansion - MEDIUM PRIORITY

**Goals**:
1. Expand LSP server (go-to-definition, hover docs)
2. Add more quantum computing examples
3. Improve error messages with suggestions
4. Add more tensor operations (slice, reshape, dot)
5. Document embedding API for Rust applications

---

## 📈 Success Metrics

### v0.6 Milestone (End of Week 4):
- [ ] Self-extracting binary on Linux
- [ ] Self-extracting binary on Windows
- [ ] TLS listener working
- [ ] HTTPS server example
- [ ] All semantics tests passing (76/76)

### v0.7 Milestone (End of Week 8):
- [ ] Native executables stable
- [ ] TLS support production-ready
- [ ] LSP with go-to-definition
- [ ] Expanded stdlib examples
- [ ] Performance benchmarks documented

### v1.0 Milestone (End of Month 6):
- [ ] Production-ready for embedded scripting
- [ ] Production-ready for web servers
- [ ] Package ecosystem functional
- [ ] Native executables stable
- [ ] Security audit passed
- [ ] Comprehensive documentation

---

## 🔧 Development Workflow

### Testing

```bash
# Run all tests
cargo test

# Run semantics tests only
cargo test --test semantics

# Run specific test
cargo test thread_stdlib_wrapper_pass

# Run with output
cargo test -- --nocapture
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run CLI
cargo run -- run examples/hello.sapp
```

### Documentation

```bash
# Generate docs
cargo doc --open

# Run sapphire doc
cargo run -- doc examples/
```

---

## 📚 Key Documentation Files

| File | Purpose |
|------|---------|
| `docs/Future_plans.md` | Vision and long-term roadmap |
| `docs/Tasks.md` | Active backlog |
| `docs/Report.md` | Progress tracking |
| `docs/IMPROVEMENT_PLAN.md` | Detailed implementation guide |
| `docs/IMPROVEMENT_STATUS.md` | Current status (this document) |
| `docs/Spec_freeze.md` | Language semantics contract |
| `docs/Registry_MVP.md` | Package system design |
| `docs/Registry_hosted.md` | Hosted registry API |
| `docs/Threads.md` | Concurrency model |
| `docs/Native_backend_notes.md` | Executable strategy |
| `docs/Tooling.md` | CLI reference |
| `docs/TLS_notes.md` | TLS design |

---

## 🎓 Lessons Learned

1. **Test-First Development Works**: 75+ semantics tests caught regressions early
2. **Documentation as Contract**: `Spec_freeze.md` provides clear semantics
3. **Incremental Milestones**: Breaking work into weekly chunks shows progress
4. **Architecture Investment**: Clean crate separation enables parallel work
5. **Capability-Based Security**: `VmLimits` + `FfiPolicy` enables safe embedding

---

## 🚀 Deployment Readiness

### Embedded Scripting (Games/Tools)

**Status**: ✅ PRODUCTION-READY

- Core VM stable
- Sandboxing works
- Error handling robust
- Memory management adequate
- Module system functional

**Use Case**: Safe scripting in Rust applications, game engines, CLI tools

### Standalone General-Purpose

**Status**: 🟡 EARLY-STAGE (Need TLS + Native Executables)

- Package ecosystem functional
- Stdlib comprehensive
- Threading working
- Missing: HTTPS, single-binary distribution

**Use Case**: Internal tools, prototyping, non-public-facing services

### Production Web Services

**Status**: ⚠️ NOT RECOMMENDED (Missing HTTPS)

- HTTP server works
- Routing/middleware functional
- Missing: TLS termination
- Workaround: Reverse proxy with nginx/Caddy

**Use Case**: Development/testing only until TLS added

---

## 📞 Contact & Support

- **GitHub Issues**: For bug reports and feature requests
- **Documentation**: See `docs/` directory
- **Examples**: See `examples/` directory
- **Tests**: See `tests/semantics/` for usage examples

---

**Last Updated**: 2026  
**Next Review**: rolling  
**Status**: Historical snapshot section ends here; use the top summary as canonical.
