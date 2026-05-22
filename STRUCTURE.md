# pblang Project Structure

```
pblang/
├── src/
│   └── main.rs                  # CLI entry point, REPL, all subcommands
│
├── crates/
│   ├── pblang-core/             # Lexer (logos), parser (chumsky), AST types
│   │   └── src/
│   │       ├── lib.rs           # AST definitions (Expr, Stmt, Span, etc.)
│   │       ├── lexer.rs         # Token definitions
│   │       └── parser.rs        # Combinator parser
│   │
│   ├── pblang-ir/               # Intermediate representation
│   │   └── src/lib.rs           # OpCode, Chunk, Value, Function, Module
│   │
│   ├── pblang-vm/               # Bytecode virtual machine
│   │   └── src/
│   │       ├── lib.rs           # Public surface, re-exports
│   │       ├── vm.rs            # Execution loop, builtins, GC
│   │       ├── chunk.rs         # Chunk utilities
│   │       ├── memory_safety.rs # Runtime ownership / borrow tracker
│   │       ├── concurrency.rs   # Mutex/RwLock/Channel/ThreadPool primitives
│   │       └── signals.rs       # OS signal handling
│   │
│   ├── pblang-compiler/         # AST → bytecode compiler + optimizer
│   │   └── src/lib.rs
│   │
│   ├── pblang-typeck/           # Type checker (inference + diagnostics)
│   │   └── src/lib.rs
│   │
│   ├── pblang-ffi/              # Foreign Function Interface (C ABI)
│   │   └── src/lib.rs           # ffi_load, ffi_call, ffi_close
│   │
│   ├── pblang-pm/               # Package manager
│   │   └── src/lib.rs           # Manifest, lockfile, semver, registry, HMAC
│   │
│   ├── pblang-lsp/              # Language Server Protocol (tower-lsp)
│   │   └── src/                 # Diagnostics, completion, hover, goto-def
│   │
│   ├── pblang-graphics/         # Graphics backend (SDL2 / minifb)
│   │   └── src/lib.rs
│   │
│   ├── pblang-cranelift/        # Cranelift JIT + AOT object emission
│   │   └── src/lib.rs
│   │
│   ├── pblang-ctranspiler/      # AST → C99 transpiler
│   │   └── src/
│   │       ├── lib.rs
│   │       └── pblang_rt.h      # Runtime header for transpiled output
│   │
│   ├── pblang-llvm/             # LLVM IR text emitter
│   │   └── src/lib.rs
│   │
│   ├── pblang-wasm/             # WAT / WebAssembly text backend
│   │   └── src/lib.rs
│   │
│   ├── pblang-ai/               # AI / ML provider integration + RAG helpers
│   │   └── src/lib.rs
│   │
│   ├── pblang-quantum/          # Bridge to Qiskit / Cirq / Braket
│   │   └── src/lib.rs
│   │
│   └── pblang-stub/             # Self-extracting bundle launcher
│       └── src/main.rs
│
├── stdlib/                      # 100+ standard library modules (.sapp)
│   ├── http.sapp, http_server.sapp, router.sapp, middleware.sapp
│   ├── websocket.sapp, tls.sapp, multipart.sapp
│   ├── json.sapp, csv.sapp, db.sapp, collections.sapp
│   ├── thread.sapp, thread_pool.sapp, concurrency.sapp, async_await.sapp
│   ├── crypto.sapp, crypto_quantum.sapp, jwt.sapp
│   ├── math.sapp, tensor.sapp, nd.sapp
│   ├── quantum.sapp, quantum_sim.sapp, quantum_viz.sapp, quantum_protocol.sapp
│   ├── fs.sapp, os.sapp, path.sapp, process.sapp, time.sapp, net.sapp
│   ├── memory.sapp                  # Wrapper around runtime memory_* builtins
│   ├── compiler/                    # Self-hosting compiler in pblang
│   │   ├── tokenizer.sapp           # Lexer
│   │   ├── parser.sapp              # Recursive-descent parser
│   │   ├── typeck.sapp              # Type checker
│   │   ├── ir.sapp, optimizer.sapp  # IR + optimizations
│   │   ├── codegen.sapp             # Bytecode code generator
│   │   ├── bootstrap.sapp           # Bootstrap driver
│   │   └── selfhost_frontend.sapp   # End-to-end self-host frontend
│   └── ... (~100 more modules: testing, orm, async, env, log, jwt, template,
│            graph, benchmark, registry, validate, regex, polyglot, ai, ...)
│
├── tests/
│   ├── semantics.rs                 # Main semantics test harness
│   ├── semantics/                   # ~95 .sapp test scripts
│   ├── common.rs                    # Shared test utilities
│   ├── cfg.rs, pack.rs, lsp.rs      # Specialized integration suites
│   ├── threads.rs, install_cli.rs   # Threading + CLI / registry tests
│   └── tensor_builtins.rs, signal_builtins.rs, ... (25+ files)
│
├── examples/
│   ├── ai_chat_toolcall/             # AI tool-call demo
│   ├── ai_http_provider/             # AI HTTP provider adapter
│   ├── quantum_bb84/                 # Quantum key distribution
│   ├── thread_pool/                  # Worker pool pattern
│   ├── self_host_demo/               # Self-hosting pipeline demo
│   └── embed.rs                      # Rust embedding example
│
├── editors/
│   └── vscode/                       # VS Code extension (TextMate + LSP)
│
├── docs/                             # Design & specification documents
│   ├── CURRENT_STATUS.md             # Component status overview
│   ├── Language_Semantics.md         # Language semantics contract
│   ├── Tasks.md                      # Active backlog
│   ├── Tooling.md, USAGE_GUIDE.md, TUTORIALS.md
│   ├── Embedding.md, Safety.md, Threads.md, TLS_notes.md
│   └── ... (28 documents)
│
├── benches/
│   └── bench_vm.rs                   # Criterion benchmarks
│
├── Cargo.toml                        # Workspace manifest
├── Cargo.lock
├── pblang.toml                       # Self-hosting package manifest
├── README.md
├── ROADMAP.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE                           # MIT
└── .gitignore
```

## Crate Dependency Graph

```
pblang (binary)
├── pblang-core         (lexer, parser, AST)
├── pblang-ir           (opcodes, values)
├── pblang-vm           (execution, builtins, GC)            ← pblang-ir
├── pblang-compiler     (codegen + optimizer)                 ← pblang-core, pblang-ir
├── pblang-typeck       (inference, diagnostics)              ← pblang-core
├── pblang-pm           (manifest, lockfile, semver, registry)
├── pblang-graphics     (SDL2 / minifb)
├── pblang-ctranspiler  (AST → C99)                           ← pblang-core, pblang-ir
├── pblang-cranelift    (JIT + AOT object files)              ← pblang-ir
├── pblang-llvm         (LLVM IR text)                        ← pblang-core, pblang-ir
├── pblang-wasm         (WAT)                                 ← pblang-core, pblang-ir
├── pblang-lsp          (tower-lsp)                           ← pblang-core, pblang-typeck
├── pblang-ai           (provider registry, RAG)
├── pblang-quantum      (Qiskit/Cirq/Braket bridge)
├── pblang-ffi          (C ABI dynamic loading)
└── pblang-stub         (self-extracting binary launcher)
```

## Key Design Decisions

1. **Crate separation** — each concern is isolated; crates can be used independently
2. **IR as shared layer** — `pblang-ir` defines the bytecode format shared by compiler, VM, and JIT/AOT backends
3. **Capability-based security** — all I/O goes through the `VmHost` trait; embedders control everything via `StdlibConfig`, `VmLimits`, and `FfiPolicy`
4. **No shared-heap threading** — each thread gets an isolated VM instance; communication via typed channels and `select`
5. **Runtime memory safety** — per-VM `MemoryTracker` enforces Rust-like borrow rules on `allocate`/`borrow_*`/`store`
6. **Polyglot-first** — source files can embed other languages via `@lang name { ... }` blocks
7. **Multi-backend codegen** — interpreter, Cranelift JIT/AOT, C99 transpile, LLVM IR, and WAT all consume the same IR
