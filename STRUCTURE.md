# pblang Project Structure

> For a complete category map of every layer (crates, backends, stdlib domains,
> kernel, tests, examples, editors, and sibling projects), see
> [PROJECT_CATEGORIES.md](docs/PROJECT_CATEGORIES.md).

```text
pblang/
├── src/
│   ├── main.rs                  # CLI entry point, REPL, command dispatch
│   └── cli/                     # CLI diagnostics, formatting, dataflow, call graph helpers
│
├── crates/
│   ├── pblang-core/             # Parser (chumsky), AST types, comments/polyglot masking
│   │   └── src/
│   │       ├── lib.rs           # AST definitions (Expr, Stmt, Span, TypeExpr, etc.)
│   │       └── parser.rs        # Combinator parser
│   │
│   ├── pblang-ir/               # Intermediate representation
│   │   └── src/lib.rs           # OpCode, Chunk, Value, Function, Module
│   │
│   ├── pblang-vm/               # Bytecode virtual machine
│   │   └── src/
│   │       ├── lib.rs           # Public surface, re-exports
│   │       ├── vm.rs            # VM state, resources, builtin registration
│   │       ├── vm/              # Builtin groups: fs, net, http, json, crypto, AI, etc.
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
│   ├── pblang-lint/             # Lint passes & diagnostics
│   │   └── src/lib.rs
│   │
│   ├── pblang-ffi/              # Foreign Function Interface helpers
│   │   └── src/lib.rs
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
├── stdlib/                      # pblang standard library modules (.sapp)
│   ├── http.sapp, http_server.sapp, router.sapp, middleware.sapp
│   ├── websocket.sapp, tls.sapp, multipart.sapp
│   ├── json.sapp, csv.sapp, db.sapp, collections.sapp
│   ├── thread.sapp, thread_pool.sapp, concurrency.sapp, async_await.sapp
│   ├── crypto.sapp, crypto_quantum.sapp, jwt.sapp
│   ├── math.sapp, tensor.sapp, nd.sapp
│   ├── browser.sapp, pbai.sapp, gui.sapp, graphics.sapp
│   ├── fs.sapp, os.sapp, path.sapp, process.sapp, time.sapp, net.sapp
│   ├── memory.sapp                  # Wrapper around runtime memory_* builtins
│   ├── compiler/                    # Self-hosting compiler in pblang
│   │   ├── tokenizer.sapp           # Lexer/tokenizer
│   │   ├── parser.sapp              # Recursive-descent parser
│   │   ├── semantic.sapp            # Semantic analysis
│   │   ├── typeck.sapp              # Type checker
│   │   ├── ir.sapp, optimizer.sapp  # IR + optimizations
│   │   ├── codegen.sapp             # Bytecode code generator
│   │   ├── vm.sapp                  # Bytecode interpreter
│   │   ├── bootstrap.sapp           # Bootstrap driver
│   │   └── selfhost_frontend.sapp   # End-to-end self-host frontend
│   └── ...
│
├── editors/
│   └── ide/                         # Native pblang IDE + legacy/web bootstrap assets
│       └── pbide.sapp               # Browser-free native IDE entry
│
├── tests/
│   ├── semantics.rs                 # Main semantics test harness
│   ├── semantics/                   # .sapp test scripts
│   ├── common.rs                    # Shared test utilities
│   ├── cfg.rs, pack.rs, lsp.rs      # Specialized integration suites
│   ├── threads.rs, install_cli.rs   # Threading + CLI / registry tests
│   └── tensor_builtins.rs, signal_builtins.rs, ...
│
├── examples/                        # Example projects
├── docs/                            # Design & specification documents
├── benches/                         # Criterion benchmarks
├── Cargo.toml                       # Workspace manifest
├── Cargo.lock
├── pblang.toml                      # pblang workspace manifest
├── README.md
├── ROADMAP.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE                          # MIT
└── .gitignore
```

## Crate Dependency Graph

```text
pblang (binary)
├── pblang-core         (parser, AST)
├── pblang-ir           (opcodes, values)
├── pblang-vm           (execution, builtins, GC)            ← pblang-ir
├── pblang-compiler     (codegen + optimizer)                 ← pblang-core, pblang-ir
├── pblang-typeck       (inference, diagnostics)              ← pblang-core
├── pblang-lint         (lints)                               ← pblang-core, pblang-typeck
├── pblang-pm           (manifest, lockfile, semver, registry)
├── pblang-graphics     (SDL2 / minifb)
├── pblang-ctranspiler  (AST → C99)                           ← pblang-core, pblang-ir
├── pblang-cranelift    (JIT + AOT object files)              ← pblang-ir
├── pblang-llvm         (LLVM IR text)                        ← pblang-core, pblang-ir
├── pblang-wasm         (WAT)                                 ← pblang-core, pblang-ir
├── pblang-lsp          (tower-lsp)                           ← pblang-core, pblang-typeck
├── pblang-ai           (provider registry, RAG)
├── pblang-quantum      (Qiskit/Cirq/Braket bridge)
├── pblang-ffi          (C ABI helpers)
└── pblang-stub         (self-extracting binary launcher)
```

## Key Design Decisions

1. **Crate separation** — each concern is isolated; crates can be used independently.
2. **IR as shared layer** — `pblang-ir` defines the bytecode format shared by compiler, VM, and JIT/AOT backends.
3. **Capability-based security** — I/O goes through host capability checks and embedder configuration.
4. **No shared-heap threading** — each thread gets an isolated VM instance; communication uses channels and handles.
5. **Runtime memory safety** — per-VM `MemoryTracker` enforces borrow/lifetime rules for the memory-safety builtins.
6. **Polyglot-first, but capability-gated** — foreign execution is useful but belongs behind `exec`/host trust boundaries.
7. **Multi-backend codegen** — interpreter is primary; JIT/AOT/C99/LLVM/WASM are backend surfaces with explicit support matrices.
