# pblang

**A fast, secure, embeddable programming language built in Rust.**

[![Version](https://img.shields.io/badge/version-0.5.7-blue)]() [![License](https://img.shields.io/badge/license-MIT-green)]() [![Tests](https://img.shields.io/badge/tests-passing-brightgreen)]()

pblang is a general-purpose programming language and bytecode VM designed for both **embedded scripting** (games, tools, applications) and **standalone development** (web servers, CLI tools, AI/quantum workflows). It features capability-based security, a built-in package manager, a standard library of ~135 modules, and a **complete self-hosted compiler** — pblang tokenizes, parses, compiles, and runs pblang, all written in pblang.

---

## ✨ Features

- **Fast Stack-Based VM** — Compiled to efficient bytecode with RC + cycle-breaking GC
- **Capability-Based Security** — Fine-grained sandboxing via `VmLimits`, `FfiPolicy`, and `StdlibConfig`
- **Runtime Memory Safety** — Per-VM `MemoryTracker` enforces Rust-like ownership, borrow, and lifetime rules at runtime (`allocate`, `borrow_immutable`, `borrow_mutable`, `borrow_unique`, `store`, `push_lifetime`)
- **Concurrency Primitives** — VM-per-thread isolation with typed channels, `select`, mutex, rwlock, thread pool, barrier, once, timer, wait group, semaphore, circuit breaker, rate limiter
- **Async / Await** — `async_spawn`, `await`, `await_all`, `await_any`, `map`, `then` built on the channel/thread runtime
- **Multi-Backend Codegen** — Interpreter, Cranelift JIT + AOT, C99 transpiler, LLVM IR, and WebAssembly text — all consume the same IR
- **Built-in Package Manager** — Semver resolution, local/remote registries, lockfiles, SHA256 + HMAC verification, token refresh
- **~135 Standard Library Modules** — HTTP/HTTPS, WebSocket, SQLite, crypto, threading, tensors, quantum, graphics, AI, ORM, JWT, template, regex, and more
- **AI / ML Integration** — `ai_chat`, `ai_embed`, `ai_token_count` with OpenAI / Anthropic / Ollama / custom providers; RAG helpers (`chunk_text`, `semantic_search`) in `pblang-ai`
- **Quantum Computing Bridge** — `quantum_run`, `quantum_statevector`, `quantum_backends` via Qiskit / Cirq / Braket subprocess
- **Polyglot Blocks** — Embed Python, JavaScript, SQL, Rust, Bash, etc. directly in source files; `polyglot_exec` for inline execution
- **FFI** — Call C libraries directly with allowlist-based security
- **TLS / HTTPS** — Production rustls + webpki-roots; `tcp_listen_tls`, `tcp_connect_tls`
- **LSP Server** — `pblang lsp` (tower-lsp): diagnostics, completions, hover, goto-def, workspace symbols, semantic tokens
- **CLI Toolchain** — `run`, `test`, `fmt`, `lint`, `build`, `bundle`, `publish`, `search`, `compile`, `pack`, `verify`, `lsp`, `--cfg`, `--symbols`, `--annotate`, `--dump-bytecode`
- **Native Binaries** — `pblang bundle --self-extract` produces a single self-extracting executable
- **Self-Hosting (complete)** — The pblang-written compiler under `stdlib/compiler/` (tokenizer → parser → semantic → typeck → ir → optimizer → codegen, plus the `selfhost_frontend.sapp` entry and `vm.sapp` bytecode interpreter) **tokenizes, parses, compiles, and runs** pblang end-to-end, entirely in pblang. Every language feature self-hosts — including `match`, `struct`/`enum`, `class` + colon-call, closures, and nil-safe operators
- **Embeddable** — Drop into any Rust application with full control over capabilities
- **Cross-Platform** — Linux, macOS, and Windows

### 🧭 North-star goals (roadmap, not yet shipped)

The language self-hosts its compiler today; the next horizons are deliberately ambitious:

- **A self-hosted IDE + visual designer written in pblang** — *pblang Studio*: drag-and-drop building of GUI, web, and CLI apps (Delphi / Qt Designer / WordPress / Joomla / Android Studio / VS Code style), with the IDE itself authored in pblang. See [docs/Visual_Designer.md](docs/Visual_Designer.md).
- **OS / bare-metal capability** — freestanding codegen + a kernel runtime so you can write (and boot) an OS in pblang. See [docs/OS_Development.md](docs/OS_Development.md).
- **pbAI** — pblang's own AI model family (a flagship + distilled sub-models), written in pblang, for coding/debugging. See [docs/pbAI.md](docs/pbAI.md).

---

## 🚀 Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)

### Build

```bash
git clone https://github.com/your-username/pblang.git
cd pblang
cargo build --release
```

### Hello World

Create `hello.sapp`:

```
print "Hello, World!";

let name = "pblang";
print "Welcome to " + name + "!";
```

Run it:

```bash
cargo run -- run hello.sapp
```

---

## 📖 Language Overview

### Variables & Types

```
let x = 42;
let name = "Alice";
let active = true;
let items = [1, 2, 3];
let config = map{host: "localhost", port: 8080};
```

### Functions

```
fn greet(name) {
    return "Hello, " + name + "!";
}

print greet("World");

// Closures
let counter = {
    let n = 0;
    fn() { n = n + 1; return n; }
};
print counter();  // 1
print counter();  // 2
```

### Control Flow

```
if x > 10 {
    print "big";
} else {
    print "small";
}

while x > 0 {
    x = x - 1;
}

for i 10 {
    print i;  // 0..9
}

for x in items {
    print x;  // for-in iteration is supported
}
```

### Match Expressions

```
let result = match status {
    case 200: "OK",
    case 404: "Not Found",
    case n if n >= 500: "Server Error",
    case _: "Unknown"
};
```

### Error Handling

```
try {
    let data = fsread("config.json");
    print data;
} catch e {
    print "Error: " + str(e);
}

defer { print "cleanup done"; }
```

### Modules & Imports

```
// Import local module
let utils = import("./utils.sapp");
print utils.format_name("Alice");

// Import from stdlib
let http = import("stdlib:http.sapp");
let thread = import("stdlib:thread.sapp");

// Export from a module
export greet;
export config;
```

### Concurrency

```
let ch = channel();

let worker = spawn(fn() {
    let msg = recv(ch);
    send(ch, msg * 2);
});

send(ch, 21);
print recv(ch);  // 42
join(worker);
```

### Polyglot Blocks

```
@python data_analysis {
    import pandas as pd
    df = pd.read_csv("data.csv")
    print(df.describe())
}

@sql migrations {
    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL
    );
}
```

---

## 🏗️ Architecture

See [STRUCTURE.md](STRUCTURE.md) for the full crate map.

```
pblang/
├── src/main.rs              # CLI entry point & REPL
├── crates/                  # 17 workspace crates
│   ├── pblang-core/         # Lexer (logos), parser (chumsky), AST
│   ├── pblang-ir/           # Intermediate representation & opcodes
│   ├── pblang-vm/           # Bytecode VM, builtins, GC, memory_safety, concurrency
│   ├── pblang-compiler/     # AST → bytecode + optimizer
│   ├── pblang-typeck/       # Type checker (inference + diagnostics)
│   ├── pblang-lint/         # Lint passes & diagnostics
│   ├── pblang-ffi/          # C ABI foreign function interface
│   ├── pblang-pm/           # Package manager (registry, lockfile, semver, HMAC)
│   ├── pblang-lsp/          # Language Server Protocol (tower-lsp)
│   ├── pblang-graphics/     # Graphics backend (SDL2 / minifb)
│   ├── pblang-cranelift/    # Cranelift JIT + AOT object emission
│   ├── pblang-ctranspiler/  # AST → C99 transpiler + runtime header
│   ├── pblang-llvm/         # LLVM IR text emitter
│   ├── pblang-wasm/         # WAT / WebAssembly backend
│   ├── pblang-ai/           # AI / ML providers + RAG helpers
│   ├── pblang-quantum/      # Qiskit / Cirq / Braket bridge
│   └── pblang-stub/         # Self-extracting bundle launcher
├── stdlib/                  # ~135 standard library modules (.sapp)
│   └── compiler/            # Self-hosting compiler in pblang itself
├── tests/                   # Semantics + integration suites
├── examples/                # Example projects
├── editors/vscode/          # VS Code extension
├── docs/                    # 37 design documents & specifications
└── benches/                 # Criterion performance benchmarks
```

### Compilation Pipeline

```
Source (.sapp) → Parser (chumsky) → AST → Type Check (pblang-typeck)
                                              ↓
                                       Compiler (pblang-compiler) + optimizer
                                              ↓
                                       Bytecode (Chunk)
                                       ╱       │        ╲
                                  Interpreter  JIT     AOT backends
                                  (pblang-vm) (cranelift) (cranelift/llvm/c99/wasm)
```

---

## 🛠️ CLI Reference

| Command | Description |
|---------|-------------|
| `pblang run <file\|dir>` | Execute a script or package |
| `pblang test [dir]` | Run test suite (default: `tests/`) |
| `pblang fmt [files...]` | Format source files |
| `pblang lint [files...]` | Parse + typecheck, report diagnostics |
| `pblang build [path]` | Generate lockfile + typecheck entry |
| `pblang init` | Create `pblang.toml` in current directory |
| `pblang new <name> [--template]` | Scaffold a new project |
| `pblang install [name@version]` | Install dependencies |
| `pblang publish` | Publish package to registry |
| `pblang search <query>` | Search package registry |
| `pblang bundle [--self-extract]` | Portable folder, or single self-extract binary (embeds `pblang` runtime) |
| `pblang pack [path]` | Create `.tar.gz` package archive |
| `pblang verify <tarball>` | Verify package integrity |
| `pblang lsp` | Start LSP server (stdin/stdout) |
| `pblang --doc <file> [--html]` | Generate documentation |
| `pblang --dump-bytecode <file>` | Print bytecode disassembly |
| `pblang --annotate <file>` | Annotated bytecode with source lines |
| `pblang --symbols <file>` | Print symbol table |
| `pblang --cfg <file>` | Dump control-flow graph |
| `pblang --check <file>` | Parse + typecheck only |

---

## 🔒 Embedding in Rust

```rust
use pblang_core::parse;
use pblang_compiler::compile;
use pblang_vm::{VM, StdlibConfig, VmLimits};

fn main() {
    let source = r#"print "Hello from embedded pblang!";"#;
    let ast = parse(source).unwrap();
    let chunk = compile(ast);

    let config = StdlibConfig {
        core: true,
        fs: false,       // No filesystem access
        net: false,      // No network access
        http: false,     // No HTTP client
        json: true,
        crypto: true,
        sqlite: false,   // No database access
        ffi: false,      // No FFI
        exec: false,     // No process execution
        gfx: false,
        spawn: true,
    };

    let mut vm = VM::with_stdlib(config);
    vm.set_limits(Some(VmLimits {
        max_instructions: Some(100_000),
        max_stack_depth: Some(64),
        ..Default::default()
    }));

    vm.run(&chunk).unwrap();
}
```

---

## 📦 Package Manager

### Manifest (`pblang.toml`)

```toml
[package]
name = "my-app"
version = "1.0.0"
entry = "src/main.sapp"
description = "My awesome application"

[dependencies]
http = { version = "^1.2.0" }
utils = { path = "./libs/utils" }
auth = { git = "https://github.com/user/auth.git" }
```

### Commands

```bash
pblang init                          # Create manifest
pblang install                       # Install all deps
pblang install @pblang/http@1.2.0   # Install specific package
pblang publish                       # Publish to registry
pblang search "http server"          # Search packages
```

---

## 📘 How To Use pblang

Start with:

- [Usage Guide](docs/USAGE_GUIDE.md) — practical language and CLI walkthrough
- [Tutorials](docs/TUTORIALS.md) — beginner mini-projects (CLI, HTTP API, concurrency)
- [Language Semantics](docs/Language_Semantics.md) — detailed language rules
- [Tooling](docs/Tooling.md) — full CLI/LSP/packaging reference
- [Current Status](docs/CURRENT_STATUS.md) — current feature maturity and known limits
- [IDE Manual](docs/IDE_MANUAL.md) — the browser code IDE (`pblang serve`); goal: a self-hosted IDE written in pblang
- [Visual Designer](docs/Visual_Designer.md) — **pblang Studio**: drag-and-drop builder for GUI, web, and CLI apps (design/roadmap)
- [OS Development](docs/OS_Development.md) — roadmap to freestanding/bare-metal pblang (kernels, drivers, boot)
- [pbAI](docs/pbAI.md) — design for pblang's native AI model family (coding/debugging agent, written in pblang)

---

## 📚 Standard Library

| Category | Modules |
|----------|---------|
| **Web** | `http`, `http_server`, `router`, `middleware`, `multipart`, `websocket`, `tls`, `networking_advanced` |
| **Data** | `json`, `csv`, `db` (SQLite), `collections`, `data_structures`, `serialization` |
| **Concurrency** | `thread`, `thread_pool`, `concurrency`, `async_await`, `event`, `signal`, `pipe` |
| **Memory Safety** | `memory` (Rust-like ownership / borrow / lifetime helpers over VM builtins) |
| **Crypto** | `crypto`, `crypto_quantum`, `jwt`, `uuid`, `bitops` |
| **Math / Science** | `math`, `tensor`, `nd`, `algorithms_advanced`, `machine_learning`, `blockchain` |
| **Quantum** | `quantum`, `quantum_sim`, `quantum_viz`, `quantum_protocol` |
| **System** | `fs`, `os`, `path`, `process`, `time`, `net`, `env`, `systems` |
| **Graphics & Imaging** | `graphics`, `image_processing`, `colors` |
| **Interop** | `ffi`, `polyglot`, `strings`, `array`, `map`, `ptr`, `units` |
| **AI / ML** | `ai`, `provider_registry` (`ai_chat`, `ai_embed`, RAG helpers via `pblang-ai`) |
| **Tooling** | `testing`, `benchmark`, `logger`, `assert`, `validate`, `template`, `regex`, `docs_generator`, `profiler`, `debugger` |
| **Package Manager** | `registry`, `registry_server` |
| **Compression** | `compression` (gzip, zstd, brotli) |
| **Self-Hosting** | `compiler/tokenizer`, `compiler/parser`, `compiler/semantic`, `compiler/typeck`, `compiler/ir`, `compiler/optimizer`, `compiler/codegen`, `compiler/vm`, `compiler/selfhost_frontend` (end-to-end run), `compiler/bootstrap` |

---

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run semantics tests only
cargo test --test semantics

# Run a specific test
cargo test block_shadowing_works

# Run with output visible
cargo test -- --nocapture
```

---

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes with tests
4. Run: `cargo test && cargo clippy`
5. Submit a pull request

See [docs/Tasks.md](docs/Tasks.md) for the active backlog, [docs/CURRENT_STATUS.md](docs/CURRENT_STATUS.md) for project status, and [CHANGELOG.md](CHANGELOG.md) for release notes.

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.
