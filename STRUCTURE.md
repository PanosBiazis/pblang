# pblang Project Structure

```
pblang/
├── src/
│   └── main.rs                  # CLI entry point, REPL, all subcommands
│
├── crates/
│   ├── pblang-core/             # Lexer, parser (chumsky), AST types
│   │   └── src/
│   │       ├── lib.rs           # AST definitions (Expr, Stmt, Span, etc.)
│   │       ├── lexer.rs         # Token definitions (logos)
│   │       └── parser.rs        # Recursive descent / combinator parser
│   │
│   ├── pblang-ir/               # Intermediate representation
│   │   └── src/
│   │       └── lib.rs           # OpCode, Chunk, Value, Function, Module, etc.
│   │
│   ├── pblang-vm/               # Bytecode virtual machine
│   │   └── src/
│   │       ├── lib.rs           # Re-exports
│   │       ├── vm.rs            # VM execution loop, builtins, GC
│   │       └── chunk.rs         # Chunk utilities
│   │
│   ├── pblang-compiler/         # AST → bytecode compiler
│   │   └── src/
│   │       └── lib.rs           # Compiler, optimizer, disassembler
│   │
│   ├── pblang-typeck/           # Type checker (basic inference)
│   │   └── src/
│   │       └── lib.rs           # Type inference, diagnostics
│   │
│   ├── pblang-ffi/              # Foreign Function Interface (C ABI)
│   │   └── src/
│   │       └── lib.rs           # ffi_load, ffi_call, ffi_close
│   │
│   ├── pblang-pm/               # Package manager
│   │   └── src/
│   │       └── lib.rs           # Manifest, lockfile, registry, semver
│   │
│   ├── pblang-lsp/              # Language Server Protocol
│   │   └── src/
│   │       └── lib.rs           # JSON-RPC, completion, hover, definition
│   │
│   └── pblang-graphics/         # Graphics backend
│       └── src/
│           └── lib.rs           # SDL2/minifb primitives
│
├── stdlib/                      # Standard library modules (.sapp)
│   ├── http.sapp                # HTTP client (get, post, streaming)
│   ├── http_server.sapp         # HTTP server (routing, middleware)
│   ├── thread.sapp              # Threading (spawn, join, channels)
│   ├── json.sapp                # JSON parse/stringify
│   ├── crypto.sapp              # SHA256, base64, random bytes
│   ├── db.sapp                  # SQLite wrapper
│   ├── quantum.sapp             # Quantum circuit builder
│   ├── tensor.sapp              # Tensor operations
│   └── ...                      # 34+ total modules
│
├── tests/
│   ├── semantics.rs             # Main semantics test harness
│   ├── semantics/               # Individual test scripts (.sapp)
│   ├── common.rs                # Shared test utilities
│   ├── cfg.rs                   # CFG dump tests
│   ├── pack.rs                  # Packaging tests
│   └── ...                      # 25+ test files
│
├── examples/
│   ├── ai_chat_toolcall/        # AI tool-call demo
│   ├── ai_http_provider/        # AI HTTP provider adapter
│   ├── quantum_bb84/            # Quantum key distribution
│   ├── thread_pool/             # Thread pool pattern
│   └── embed.rs                 # Rust embedding example
│
├── editors/
│   └── vscode/                  # VS Code extension scaffold
│       ├── package.json
│       ├── syntaxes/            # TextMate grammar
│       └── src/extension.ts     # LSP client
│
├── docs/                        # Design & specification documents
│   ├── CURRENT_STATUS.md        # Component status overview
│   ├── Language_Semantics.md    # Language semantics contract
│   ├── Tasks.md                 # Active development backlog
│   ├── Tooling.md               # CLI & stdlib reference
│   └── ...                      # 20+ design documents
│
├── benches/                     # Criterion benchmarks
│   └── bench_vm.rs
│
├── Cargo.toml                   # Workspace manifest
├── Cargo.lock                   # Dependency lockfile
├── pblang.toml                  # pblang package manifest (self-hosting)
├── README.md                    # Project documentation
├── ROADMAP.md                   # Version milestones
├── LICENSE                      # MIT license
└── .gitignore
```

## Crate Dependency Graph

```
pblang (binary)
├── pblang-core      (parser, AST)
├── pblang-ir        (opcodes, values)
├── pblang-vm        (execution)    ← depends on pblang-ir
├── pblang-compiler  (codegen)      ← depends on pblang-core, pblang-ir
├── pblang-typeck    (type check)   ← depends on pblang-core
├── pblang-pm        (packages)
└── pblang-graphics  (rendering)
```

## Key Design Decisions

1. **Crate separation** — Each concern is isolated; crates can be used independently
2. **IR as shared layer** — `pblang-ir` defines the bytecode format shared by compiler and VM
3. **Capability-based** — All I/O goes through `VmHost` trait; embedders control everything
4. **No shared-heap threading** — Each thread gets an isolated VM instance
5. **Polyglot-first** — Source files can embed other languages via `@lang { ... }` blocks
