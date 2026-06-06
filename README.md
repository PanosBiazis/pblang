# pblang

**A fast, secure, embeddable programming language built in Rust.**

[![Version](https://img.shields.io/badge/version-0.5.7-blue)]() [![License](https://img.shields.io/badge/license-MIT-green)]() [![Tests](https://img.shields.io/badge/tests-passing-brightgreen)]()

pblang is a general-purpose programming language and bytecode VM designed for embedded scripting and standalone tooling. The project includes a Rust compiler/VM toolchain, a pblang standard library, package tooling, LSP support, a native pblang IDE, and a self-hosted compiler written in pblang.

---

## What works today

### Stable / primary surface

- **Stack-based bytecode VM** with closures, modules, errors, arrays/maps, GC support, and runtime diagnostics.
- **Parser, compiler, type checker, linter, formatter, and CLI**.
- **Capability-based embedding controls** through `StdlibConfig`, `VmLimits`, and `FfiPolicy`.
- **Package manager** with manifests, lockfiles, local and registry dependencies, checksums, and HMAC helpers.
- **Standard library** for files, paths, JSON, strings, arrays, maps, HTTP helpers, SQLite, crypto, threading, async-style helpers, testing, templates, logging, regex, tensors, and more.
- **LSP server** with diagnostics, completions, hover, goto-definition, workspace symbols, and semantic tokens.
- **Native pblang IDE** via `pblang --ide`: code editor, terminal, visual designer, pbAI panel, and pblang-native browser panel.
- **Self-hosted compiler pipeline** under `stdlib/compiler/`, including tokenizer, parser, semantic analysis, type checking, IR, optimizer, codegen, and a pblang bytecode interpreter.

### Experimental / preview surface

- **Cranelift JIT** for eligible numeric functions, with fallback to the bytecode VM.
- **C99 transpiler, LLVM IR emitter, and WAT/WASM backend**. These are useful development backends, but not all language features are fully covered.
- **AI and quantum integrations**. These are bridges and helpers, not the minimal core language.
- **Polyglot execution**. Useful for trusted workflows; keep it behind `exec` capability.

### Roadmap / research goals

- A fully self-hosted visual Studio written in pblang.
- Bare-metal / OS-development support.
- pbAI as a native pblang model/tooling family.

---

## Quick start

### Prerequisites

- Rust 1.70+

### Build

```bash
git clone https://github.com/PanosBiazis/pblang.git
cd pblang
cargo build --release
```

### Hello World

Create `hello.sapp`:

```pblang
print "Hello, World!";

fn add(a, b) {
    return a + b;
}

print add(2, 3);
```

Run it:

```bash
cargo run -- run hello.sapp
```

---

## Common commands

| Command | Description |
|---|---|
| `pblang run <file|dir>` | Run a script or package. |
| `pblang test [dir]` | Run pblang tests. |
| `pblang fmt [files...]` | Format source files. |
| `pblang lint [files...]` | Parse, type-check, and lint files. |
| `pblang build [path]` | Generate lockfile and type-check the package entry. |
| `pblang init` | Create a `pblang.toml`. |
| `pblang new <name>` | Scaffold a new project. |
| `pblang install` | Install dependencies. |
| `pblang publish` | Publish a package. |
| `pblang search <query>` | Search the package registry. |
| `pblang bundle [--self-extract]` | Build a portable bundle or self-extracting executable. |
| `pblang lsp` | Start the language server. |
| `pblang --ide` | Launch the native pblang IDE. |
| `pblang --check <file>` | Parse and type-check only. |
| `pblang --dump-bytecode <file>` | Print bytecode disassembly. |
| `pblang --cfg <file>` | Dump a control-flow graph. |

---

## Language overview

```pblang
let name = "Alice";
let nums = [1, 2, 3];
let config = map{host: "localhost", port: 8080};

fn greet(name) {
    return "Hello, " + name + "!";
}

if len(nums) > 0 {
    print greet(name);
}

for x in nums {
    print x;
}

let result = match 200 {
    case 200: "OK",
    case 404: "Not Found",
    case _: "Unknown"
};

print result;
```

---

## Project layout

```text
pblang/
├── src/main.rs              # CLI entry point and command dispatch
├── src/cli/                 # CLI helpers and reports
├── crates/                  # Rust workspace crates
│   ├── pblang-core/         # Parser and AST
│   ├── pblang-ir/           # Bytecode, opcodes, values
│   ├── pblang-vm/           # VM, builtins, GC, sandbox hooks
│   ├── pblang-compiler/     # AST → bytecode compiler
│   ├── pblang-typeck/       # Type checker
│   ├── pblang-lint/         # Lints
│   ├── pblang-pm/           # Package manager
│   ├── pblang-lsp/          # Language server
│   ├── pblang-cranelift/    # JIT / AOT prototype backend
│   ├── pblang-ctranspiler/  # C99 transpiler
│   ├── pblang-llvm/         # LLVM IR emitter
│   ├── pblang-wasm/         # WAT/WASM emitter
│   ├── pblang-ai/           # AI provider helpers
│   ├── pblang-quantum/      # Quantum bridge
│   └── pblang-stub/         # Self-extracting bundle launcher
├── stdlib/                  # pblang standard library modules
├── stdlib/compiler/         # self-hosted compiler written in pblang
├── editors/ide/             # native pblang IDE
├── tests/                   # Rust and pblang tests
├── examples/                # example projects
├── docs/                    # design, safety, tooling, and status docs
└── benches/                 # benchmarks
```

See [`STRUCTURE.md`](STRUCTURE.md) for the crate map and [`docs/CURRENT_STATUS.md`](docs/CURRENT_STATUS.md) for the current test/status snapshot.

---

## Embedding safely in Rust

```rust
use pblang_core::parse;
use pblang_compiler::compile;
use pblang_vm::{StdlibConfig, VmLimits, VM};

fn main() {
    let source = r#"print "Hello from embedded pblang!";"#;
    let ast = parse(source).unwrap();
    let chunk = compile(ast).unwrap();

    let config = StdlibConfig {
        core: true,
        fs: false,
        net: false,
        http: false,
        json: true,
        crypto: true,
        sqlite: false,
        ffi: false,
        exec: false,
        gfx: false,
        spawn: false,
        env: false,
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

Use `fs`, `net`, `http`, `exec`, `ffi`, and `env` only for trusted scripts or inside an OS-level sandbox.

---

## Documentation

- [`docs/USAGE_GUIDE.md`](docs/USAGE_GUIDE.md)
- [`docs/TUTORIALS.md`](docs/TUTORIALS.md)
- [`docs/Language_Semantics.md`](docs/Language_Semantics.md)
- [`docs/Tooling.md`](docs/Tooling.md)
- [`docs/Safety.md`](docs/Safety.md)
- [`docs/Capability_model.md`](docs/Capability_model.md)
- [`docs/PB_IDE.md`](docs/PB_IDE.md)
- [`docs/Visual_Designer.md`](docs/Visual_Designer.md)
- [`docs/CURRENT_STATUS.md`](docs/CURRENT_STATUS.md)
- [`docs/Tasks.md`](docs/Tasks.md)

---

## Testing

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --workspace --examples
```

---

## License

MIT. See [`LICENSE`](LICENSE).
