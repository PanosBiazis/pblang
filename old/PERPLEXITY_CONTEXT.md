# Biapryx – project context for Perplexity

**How to use:** Copy this file (and any code snippets you need) and paste them into [Perplexity](https://www.perplexity.ai) when asking questions. No API key required.

---

## What this project is

**Biapryx** is a programming language implementation written in Rust. It is a workspace with multiple crates: parser, IR, VM, compiler, type checker, package manager, LSP, FFI, and graphics.

## Workspace layout

- **biapryx-core** – Parser, lexer, core language/ast
- **biapryx-ir** – Intermediate representation
- **biapryx-vm** – Bytecode VM (chunk, VM execution)
- **biapryx-compiler** – Compilation pipeline
- **biapryx-typeck** – Type checking
- **biapryx-ffi** – FFI layer
- **biapryx-pm** – Package manager
- **biapryx-lsp** – Language server (tower-lsp)
- **biapryx-graphics** – Graphics (SDL2 / minifb), `.biapryx` examples

Other crates: `biapryx-ai`, `biapryx-llvm` (Cargo.toml only in the list).

Root `Cargo.toml` depends on: `biapryx-core`, `biapryx-vm`, `biapryx-compiler`, `biapryx-typeck`, `biapryx-pm`, `biapryx-graphics`, plus `chumsky`, `libloading`. Entrypoint is `src/main.rs`.

## Config

- **biapryx.toml** – Workspace/package config (e.g. `stdlib = { path = "stdlib" }`).
- **stdlib/** – Standard library (path dependency in biapryx.toml).

## Tests (root)

- `tests/parser_ast.rs`, `smoke.rs`, `vm_smoke.rs`, `red_features.rs`, `print_collect.rs`, `block_expr.rs`, `if_else.rs`, `semantics.rs`, `modules.rs`, `http_server.rs`, `sandbox.rs`, `golden.rs`, `lockfile_repro.rs`, `stdlib_parse.rs`, `fmt.rs`, `pack.rs`, `install_cli.rs`, `threads.rs`, `dump_bytecode.rs`, `doc_cli.rs`

Run: `cargo test`. See [README.md](README.md) for command summary and doc links.

## Key source files

- Parser/lexer: `crates/biapryx-core/src/parser.rs`, `lexer.rs`
- VM: `crates/biapryx-vm/src/vm.rs`, `chunk.rs`
- Compiler: `crates/biapryx-compiler/src/lib.rs`
- Type checker: `crates/biapryx-typeck/src/lib.rs`
- LSP: `crates/biapryx-lsp/src/main.rs`

When asking about a specific area, paste the relevant file(s) below this context in the same Perplexity message.
