# pblang

**A fast, secure, embeddable programming language built in Rust.**

[![Version](https://img.shields.io/badge/version-0.5.1-blue)]() [![License](https://img.shields.io/badge/license-MIT-green)]() [![Tests](https://img.shields.io/badge/tests-135%2F135%20passing-brightgreen)]()

pblang is a general-purpose programming language and bytecode VM designed for both **embedded scripting** (games, tools, applications) and **standalone development** (web servers, CLI tools, AI/quantum workflows). It features capability-based security, a built-in package manager, and a growing standard library of 34+ modules.

---

## ✨ Features

- **Fast Stack-Based VM** — Compiled to efficient bytecode with RC + cycle-breaking GC
- **Capability-Based Security** — Fine-grained sandboxing via `VmLimits`, `FfiPolicy`, and `StdlibConfig`
- **Built-in Package Manager** — Semver resolution, local/remote registries, lockfiles, SHA256 verification
- **68 Standard Library Modules** — HTTP, WebSocket, SQLite, crypto, threading, tensors, quantum, graphics, and more
- **Concurrency** — VM-per-thread model with typed channels and multi-channel `select`
- **Polyglot Blocks** — Embed Python, JavaScript, SQL, or any language directly in source files
- **FFI** — Call C libraries directly with allowlist-based security
- **LSP Server** — `pblang lsp` uses tower-lsp (`pblang-lsp`): parse + typecheck diagnostics, completions, hover, go-to-definition, workspace symbols
- **CLI Toolchain** — `run`, `test`, `fmt`, `lint`, `build`, `bundle`, `publish`, and more
- **Embeddable** — Drop into any Rust application with full control over capabilities
- **Cross-Platform** — Linux, macOS, and Windows

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
    print i;  // 0..9  // NOTE: for supports 0..N range only; for x in array planned for v0.7
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

```
pblang/
├── src/main.rs              # CLI entry point & REPL
├── crates/
│   ├── pblang-core/         # Lexer, parser, AST definitions
│   ├── pblang-ir/           # Intermediate representation & opcodes
│   ├── pblang-vm/           # Bytecode VM, builtins, GC
│   ├── pblang-compiler/     # AST → bytecode compilation + optimizer
│   ├── pblang-typeck/       # Type checker (basic inference)
│   ├── pblang-ffi/          # C ABI foreign function interface
│   ├── pblang-pm/           # Package manager (registry, lockfile, semver)
│   ├── pblang-lsp/          # Language Server Protocol implementation
│   └── pblang-graphics/     # Graphics backend (SDL2 / minifb)
├── stdlib/                  # 34+ standard library modules (.sapp)
├── tests/                   # Integration & semantics test suite
├── examples/                # Example projects
├── editors/vscode/          # VS Code extension
├── docs/                    # Design documents & specifications
└── benches/                 # Performance benchmarks
```

### Compilation Pipeline

```
Source (.sapp) → Parser (chumsky) → AST → Type Check → Compiler → Bytecode (Chunk) → VM Execution
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
        allow_fs: false,      // No filesystem access
        allow_net: false,     // No network access
        allow_http: false,    // No HTTP client
        allow_ffi: false,     // No FFI
        allow_exec: false,    // No process execution
        allow_sqlite: false,  // No database access
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

## 📚 Standard Library

| Category | Modules |
|----------|---------|
| **Web** | `http`, `http_server`, `router`, `middleware`, `multipart`, `websocket`, `tls` |
| **Data** | `json`, `csv`, `db` (SQLite), `collections` |
| **Concurrency** | `thread` |
| **Crypto** | `crypto`, `crypto_quantum` |
| **Math/Science** | `math`, `tensor`, `nd`, `quantum`, `quantum_sim`, `quantum_viz`, `quantum_protocol` |
| **System** | `fs`, `os`, `path`, `process`, `time`, `net` |
| **Graphics** | `graphics` |
| **Interop** | `ffi`, `polyglot`, `strings`, `array`, `map`, `ptr` |
| **Testing** | `testing` |
| **Async** | `async` |
| **ORM** | `orm` |
| **Logging** | `log` |
| **Templating** | `template` |
| **Auth** | `jwt` |
| **Config** | `env` (`.env` loader) |

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

See [docs/Tasks.md](docs/Tasks.md) for the active backlog and [docs/CURRENT_STATUS.md](docs/CURRENT_STATUS.md) for project status.

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.
